use crate::config::ServiceConfig;
use crate::errors::ServiceError;
use crate::pattern::PatternMatcher;
use crate::rules::{RuleEvaluator, RuleSearchParam, parse_rule_config};
use crate::types::*;
use ast_grep_language::SupportLang as Language;
// Removed unused imports
use globset::{Glob, GlobSetBuilder};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use walkdir::WalkDir;

#[derive(Clone)]
pub struct SearchService {
    config: ServiceConfig,
    pattern_matcher: PatternMatcher,
    rule_evaluator: RuleEvaluator,
}

impl SearchService {
    pub fn new(
        config: ServiceConfig,
        pattern_matcher: PatternMatcher,
        rule_evaluator: RuleEvaluator,
    ) -> Self {
        Self {
            config,
            pattern_matcher,
            rule_evaluator,
        }
    }

    pub async fn search(&self, param: SearchParam) -> Result<SearchResult, ServiceError> {
        let lang = Language::from_str(&param.language)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        let matches = self
            .pattern_matcher
            .search(&param.code, &param.pattern, lang)?;

        Ok(SearchResult { matches })
    }

    pub async fn file_search(
        &self,
        param: FileSearchParam,
    ) -> Result<FileSearchResult, ServiceError> {
        // Early return if cursor indicates completion
        if let Some(ref cursor) = param.cursor {
            if cursor.is_complete {
                return Ok(FileSearchResult {
                    matches: vec![],
                    next_cursor: Some(CursorResult {
                        last_file_path: String::new(),
                        is_complete: true,
                    }),
                    total_files_found: 0,
                });
            }
        }

        let lang = Language::from_str(&param.language)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        let glob = Glob::new(&param.path_pattern)
            .map_err(|e| ServiceError::Internal(format!("Invalid glob pattern: {e}")))?;
        let mut glob_builder = GlobSetBuilder::new();
        glob_builder.add(glob);
        let glob_set = glob_builder
            .build()
            .map_err(|e| ServiceError::Internal(format!("Failed to build glob set: {e}")))?;

        // Collect all potential files first
        let all_files: Vec<(String, u64)> = self.config.root_directories
            .iter()
            .flat_map(|root_dir| {
                WalkDir::new(root_dir)
                    .max_depth(10)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .filter_map(|entry| {
                        let path = entry.path();
                        let path_str = path.to_string_lossy().to_string();
                        
                        // Check if matches glob pattern
                        if !glob_set.is_match(&path_str) {
                            return None;
                        }
                        
                        // Check file size
                        entry.metadata()
                            .ok()
                            .filter(|m| m.len() <= param.max_file_size)
                            .map(|m| (path_str, m.len()))
                    })
            })
            .collect();

        // Sort files for consistent pagination
        let mut sorted_files = all_files;
        sorted_files.sort_by(|a, b| a.0.cmp(&b.0));

        // Apply cursor filtering and process files
        let cursor_filter = param.cursor.as_ref().map(|c| c.last_file_path.clone());
        
        let file_results: Vec<FileMatchResult> = sorted_files
            .into_iter()
            .filter(|(path, _)| {
                cursor_filter.as_ref().map_or(true, |start| path > start)
            })
            .filter_map(|(path_str, file_size)| {
                // Read file and search for matches
                std::fs::read_to_string(&path_str)
                    .ok()
                    .and_then(|content| {
                        self.pattern_matcher
                            .search(&content, &param.pattern, lang)
                            .ok()
                            .and_then(|matches| {
                                if matches.is_empty() {
                                    None
                                } else {
                                    let file_hash = format!("{:x}", Sha256::digest(content.as_bytes()));
                                    Some(FileMatchResult {
                                        file_path: path_str,
                                        file_size_bytes: file_size,
                                        matches,
                                        file_hash,
                                    })
                                }
                            })
                    })
            })
            .take(param.max_results)
            .collect();

        let total_files_found = file_results.len();
        let is_complete = file_results.len() < param.max_results;
        let last_path = file_results.last().map(|r| r.file_path.clone()).unwrap_or_default();

        Ok(FileSearchResult {
            matches: file_results,
            next_cursor: Some(CursorResult {
                last_file_path: if is_complete { String::new() } else { last_path },
                is_complete,
            }),
            total_files_found,
        })
    }

    pub async fn rule_search(
        &self,
        param: RuleSearchParam,
    ) -> Result<FileSearchResult, ServiceError> {
        // Check if cursor indicates completion
        if let Some(ref cursor) = param.cursor {
            if cursor.is_complete {
                return Ok(FileSearchResult {
                    matches: vec![],
                    next_cursor: Some(CursorResult {
                        last_file_path: String::new(),
                        is_complete: true,
                    }),
                    total_files_found: 0,
                });
            }
        }
        
        let rule = parse_rule_config(&param.rule_config)?;
        let lang = Language::from_str(&rule.language)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        // Use path pattern or default to all files
        let path_pattern = param.path_pattern.unwrap_or_else(|| "**/*".to_string());

        let glob = Glob::new(&path_pattern)
            .map_err(|e| ServiceError::Internal(format!("Invalid glob pattern: {e}")))?;
        let mut glob_builder = GlobSetBuilder::new();
        glob_builder.add(glob);
        let glob_set = glob_builder
            .build()
            .map_err(|e| ServiceError::Internal(format!("Failed to build glob set: {e}")))?;

        let mut file_results = Vec::new();
        let mut total_files_found = 0;
        let mut files_processed = 0;

        // Determine starting point for pagination
        let start_after = param.cursor.as_ref().map(|c| c.last_file_path.clone());

        for root_dir in &self.config.root_directories {
            for entry in WalkDir::new(root_dir)
                .max_depth(10)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let path = entry.path();
                let path_str = path.to_string_lossy();

                // Skip until we reach the cursor position
                if let Some(ref start_path) = start_after {
                    if path_str.as_ref() <= start_path.as_str() {
                        continue;
                    }
                }

                if glob_set.is_match(path_str.as_ref()) {
                    // Check file size
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.len() > param.max_file_size {
                            continue;
                        }
                    }

                    // Read and search file
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let matches = self
                            .rule_evaluator
                            .evaluate_rule_against_code(&rule.rule, &content, lang)?;

                        if !matches.is_empty() {
                            total_files_found += 1;
                            let file_hash = format!("{:x}", Sha256::digest(content.as_bytes()));

                            file_results.push(FileMatchResult {
                                file_path: path_str.to_string(),
                                file_size_bytes: content.len() as u64,
                                matches,
                                file_hash,
                            });

                            files_processed += 1;

                            // Check if we've reached the limit
                            if files_processed >= param.max_results {
                                let next_cursor = Some(CursorResult {
                                    last_file_path: path_str.to_string(),
                                    is_complete: false,
                                });

                                return Ok(FileSearchResult {
                                    matches: file_results,
                                    next_cursor,
                                    total_files_found,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(FileSearchResult {
            matches: file_results,
            next_cursor: Some(CursorResult {
                last_file_path: String::new(),
                is_complete: true,
            }),
            total_files_found,
        })
    }
}
