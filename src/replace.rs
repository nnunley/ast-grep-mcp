use crate::config::ServiceConfig;
use crate::errors::ServiceError;
use crate::pattern::PatternMatcher;
use crate::rules::{RuleEvaluator, RuleReplaceParam, parse_rule_config};
use crate::types::*;
use ast_grep_core::AstGrep;
use ast_grep_language::SupportLang as Language;
use globset::{Glob, GlobSetBuilder};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::str::FromStr;
use walkdir::WalkDir;

#[derive(Clone)]
pub struct ReplaceService {
    config: ServiceConfig,
    pattern_matcher: PatternMatcher,
    rule_evaluator: RuleEvaluator,
}

impl ReplaceService {
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

    pub async fn replace(&self, param: ReplaceParam) -> Result<ReplaceResult, ServiceError> {
        let lang = Language::from_str(&param.language)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        // First, find all matches to track changes
        let matches = self
            .pattern_matcher
            .search(&param.code, &param.pattern, lang)?;

        // Apply the replacement
        let new_code =
            self.pattern_matcher
                .replace(&param.code, &param.pattern, &param.replacement, lang)?;

        // Convert matches to change results
        let changes: Vec<ChangeResult> = matches
            .into_iter()
            .map(|m| ChangeResult {
                start_line: m.start_line,
                end_line: m.end_line,
                start_col: m.start_col,
                end_col: m.end_col,
                old_text: m.text,
                new_text: param.replacement.clone(), // Simplified - in reality would need template substitution
            })
            .collect();

        Ok(ReplaceResult { new_code, changes })
    }

    pub async fn file_replace(
        &self,
        param: FileReplaceParam,
    ) -> Result<FileReplaceResult, ServiceError> {
        // Check if cursor indicates completion
        if let Some(ref cursor) = param.cursor {
            if cursor.is_complete {
                return Ok(FileReplaceResult {
                    file_results: vec![],
                    summary_results: vec![],
                    next_cursor: Some(CursorResult {
                        last_file_path: String::new(),
                        is_complete: true,
                    }),
                    total_files_found: 0,
                    dry_run: param.dry_run,
                    total_changes: 0,
                    files_with_changes: 0,
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
        let all_files: Vec<(String, PathBuf, u64)> = self
            .config
            .root_directories
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
                        entry
                            .metadata()
                            .ok()
                            .filter(|m| m.len() <= param.max_file_size)
                            .map(|m| (path_str, path.to_path_buf(), m.len()))
                    })
            })
            .collect();

        // Sort files for consistent pagination
        let mut sorted_files = all_files;
        sorted_files.sort_by(|a, b| a.0.cmp(&b.0));

        // Apply cursor filtering
        let cursor_filter = param.cursor.as_ref().map(|c| c.last_file_path.clone());

        // Process files and collect results
        let mut file_results = Vec::new();
        let mut total_files_found = 0;
        let mut total_changes = 0;
        let mut files_with_changes = 0;

        for (path_str, path_buf, file_size) in sorted_files
            .into_iter()
            .filter(|(path, _, _)| cursor_filter.as_ref().is_none_or(|start| path > start))
        {
            // Read file content
            let content = match std::fs::read_to_string(&path_buf) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Search for matches
            let matches = match self.pattern_matcher.search(&content, &param.pattern, lang) {
                Ok(m) if !m.is_empty() => m,
                _ => continue,
            };

            // Apply replacements
            total_files_found += 1;
            let new_content =
                self.pattern_matcher
                    .replace(&content, &param.pattern, &param.replacement, lang)?;
            let file_hash = format!("{:x}", Sha256::digest(content.as_bytes()));

            // Convert matches to changes
            let changes: Vec<ChangeResult> = matches
                .into_iter()
                .map(|m| ChangeResult {
                    start_line: m.start_line,
                    end_line: m.end_line,
                    start_col: m.start_col,
                    end_col: m.end_col,
                    old_text: m.text,
                    new_text: param.replacement.clone(),
                })
                .collect();

            let change_count = changes.len();
            total_changes += change_count;
            files_with_changes += 1;

            // Write file if not dry run
            if !param.dry_run && change_count > 0 {
                std::fs::write(&path_buf, new_content)?;
            }

            file_results.push(FileDiffResult {
                file_path: path_str.clone(),
                file_size_bytes: file_size,
                changes,
                total_changes: change_count,
                file_hash,
            });

            // Check if we've reached the limit
            if file_results.len() >= param.max_results {
                let next_cursor = Some(CursorResult {
                    last_file_path: path_str,
                    is_complete: false,
                });

                return self.build_file_replace_result(
                    param,
                    file_results,
                    next_cursor,
                    total_files_found,
                    total_changes,
                    files_with_changes,
                );
            }
        }

        let next_cursor = Some(CursorResult {
            last_file_path: String::new(),
            is_complete: true,
        });

        self.build_file_replace_result(
            param,
            file_results,
            next_cursor,
            total_files_found,
            total_changes,
            files_with_changes,
        )
    }

    pub async fn rule_replace(
        &self,
        param: RuleReplaceParam,
    ) -> Result<FileReplaceResult, ServiceError> {
        // Check if cursor indicates completion
        if let Some(ref cursor) = param.cursor {
            if cursor.is_complete {
                return Ok(FileReplaceResult {
                    file_results: vec![],
                    summary_results: vec![],
                    next_cursor: Some(CursorResult {
                        last_file_path: String::new(),
                        is_complete: true,
                    }),
                    total_files_found: 0,
                    dry_run: param.dry_run,
                    total_changes: 0,
                    files_with_changes: 0,
                });
            }
        }

        let rule = parse_rule_config(&param.rule_config)?;

        if rule.fix.is_none() {
            return Err(ServiceError::ParserError(
                "Rule must have a 'fix' field for replacement".to_string(),
            ));
        }

        let fix_template = rule.fix.unwrap();
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
        let mut total_changes = 0;
        let mut files_with_changes = 0;

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

                    // Read and process file
                    if let Ok(content) = std::fs::read_to_string(path) {
                        // Apply rule-based replacement
                        let _ast = AstGrep::new(&content, lang);

                        // For rule-based replacement, we would need to implement proper pattern matching
                        // and template substitution. For now, this is a simplified version.
                        let matches = self
                            .rule_evaluator
                            .evaluate_rule_against_code(&rule.rule, &content, lang)?;

                        if !matches.is_empty() {
                            total_files_found += 1;
                            let file_hash = format!("{:x}", Sha256::digest(content.as_bytes()));

                            // Convert matches to changes (simplified)
                            let changes: Vec<ChangeResult> = matches
                                .into_iter()
                                .map(|m| ChangeResult {
                                    start_line: m.start_line,
                                    end_line: m.end_line,
                                    start_col: m.start_col,
                                    end_col: m.end_col,
                                    old_text: m.text.clone(),
                                    new_text: fix_template.clone(), // Simplified - would need proper template substitution
                                })
                                .collect();

                            let change_count = changes.len();
                            total_changes += change_count;

                            if change_count > 0 {
                                files_with_changes += 1;
                            }

                            file_results.push(FileDiffResult {
                                file_path: path_str.to_string(),
                                file_size_bytes: content.len() as u64,
                                changes,
                                total_changes: change_count,
                                file_hash,
                            });

                            // Check if we've reached the limit
                            if file_results.len() >= param.max_results {
                                let next_cursor = Some(CursorResult {
                                    last_file_path: path_str.to_string(),
                                    is_complete: false,
                                });

                                return Ok(FileReplaceResult {
                                    file_results,
                                    summary_results: vec![],
                                    next_cursor,
                                    total_files_found,
                                    dry_run: param.dry_run,
                                    total_changes,
                                    files_with_changes,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(FileReplaceResult {
            file_results,
            summary_results: vec![],
            next_cursor: Some(CursorResult {
                last_file_path: String::new(),
                is_complete: true,
            }),
            total_files_found,
            dry_run: param.dry_run,
            total_changes,
            files_with_changes,
        })
    }

    fn build_file_replace_result(
        &self,
        param: FileReplaceParam,
        file_results: Vec<FileDiffResult>,
        next_cursor: Option<CursorResult>,
        total_files_found: usize,
        total_changes: usize,
        files_with_changes: usize,
    ) -> Result<FileReplaceResult, ServiceError> {
        if param.summary_only {
            // Convert to summary results
            let summary_results: Vec<FileSummaryResult> = file_results
                .into_iter()
                .map(|diff_result| {
                    let sample_changes = if param.include_samples {
                        diff_result
                            .changes
                            .into_iter()
                            .take(param.max_samples)
                            .collect()
                    } else {
                        vec![]
                    };

                    FileSummaryResult {
                        file_path: diff_result.file_path,
                        file_size_bytes: diff_result.file_size_bytes,
                        total_changes: diff_result.total_changes,
                        lines_changed: diff_result.total_changes, // For now, assume 1:1 mapping
                        file_hash: diff_result.file_hash,
                        sample_changes,
                    }
                })
                .collect();

            Ok(FileReplaceResult {
                file_results: vec![],
                summary_results,
                next_cursor,
                total_files_found,
                dry_run: param.dry_run,
                total_changes,
                files_with_changes,
            })
        } else {
            Ok(FileReplaceResult {
                file_results,
                summary_results: vec![],
                next_cursor,
                total_files_found,
                dry_run: param.dry_run,
                total_changes,
                files_with_changes,
            })
        }
    }
}
