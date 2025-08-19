use crate::config::ServiceConfig;
use crate::errors::ServiceError;
use crate::path_validation::validate_path_pattern;
use crate::pattern::PatternMatcher;
use crate::rules::{RuleEvaluator, RuleSearchParam, parse_rule_config};
use crate::types::*;

use ast_grep_language::SupportLang as Language;
use globset::{Glob, GlobSetBuilder};
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

    /// Discovers and filters files based on a path pattern, size limits, and pagination cursor.
    /// Returns a tuple of (filtered_file_paths, next_cursor, total_files_found).
    async fn find_and_filter_files(
        &self,
        path_pattern: &str,
        max_file_size: u64,
        max_results: usize,
        cursor: Option<CursorParam>,
    ) -> Result<(Vec<(String, u64)>, Option<CursorResult>, usize), ServiceError> {
        // Early return if cursor indicates completion
        if let Some(ref c) = cursor {
            if c.is_complete {
                return Ok((
                    vec![],
                    Some(CursorResult {
                        last_file_path: String::new(),
                        is_complete: true,
                    }),
                    0,
                ));
            }
        }

        let validated_pattern = validate_path_pattern(path_pattern)?;

        // Check if this is a direct file path (not a glob pattern)
        let path = std::path::Path::new(&validated_pattern);
        if path.is_file() {
            // Validate the file is under a root directory
            self.validate_file_under_roots(&validated_pattern)?;

            // Get file metadata
            let metadata = std::fs::metadata(&validated_pattern).map_err(|e| {
                ServiceError::Internal(format!("Failed to read file metadata: {e}"))
            })?;

            let file_paths = if metadata.len() <= max_file_size {
                vec![(validated_pattern.clone(), metadata.len())]
            } else {
                vec![]
            };

            let total_files = file_paths.len();
            let next_cursor = Some(CursorResult {
                last_file_path: validated_pattern.clone(),
                is_complete: true,
            });

            return Ok((file_paths, next_cursor, total_files));
        }

        let glob = Glob::new(&validated_pattern)
            .map_err(|e| ServiceError::Internal(format!("Invalid glob pattern: {e}")))?;
        let mut glob_builder = GlobSetBuilder::new();
        glob_builder.add(glob);
        let glob_set = glob_builder
            .build()
            .map_err(|e| ServiceError::Internal(format!("Failed to build glob set: {e}")))?;

        let glob_set = std::sync::Arc::new(glob_set);

        // Determine search roots and pattern
        let (search_roots, effective_glob_pattern) = if validated_pattern.starts_with('/') {
            // Absolute path pattern - check if it's within any root
            let mut found_root = None;
            let mut relative_pattern = None;

            // Try to extract the directory part of the pattern to canonicalize it
            let pattern_parts: Vec<&str> = validated_pattern.split('/').collect();
            let mut dir_end_idx = pattern_parts.len();

            // Find where the glob pattern starts (contains *, ?, [, etc.)
            for (i, part) in pattern_parts.iter().enumerate() {
                if part.contains('*') || part.contains('?') || part.contains('[') {
                    dir_end_idx = i;
                    break;
                }
            }

            // Build the directory path and glob suffix
            let dir_path = pattern_parts[..dir_end_idx].join("/");
            let glob_suffix = if dir_end_idx < pattern_parts.len() {
                pattern_parts[dir_end_idx..].join("/")
            } else {
                String::new()
            };

            // Try to canonicalize the directory part
            let canonical_pattern_dir =
                if let Ok(canonical) = std::path::Path::new(&dir_path).canonicalize() {
                    canonical.to_string_lossy().to_string()
                } else {
                    dir_path
                };

            for root in &self.config.root_directories {
                if let Ok(canonical_root) = root.canonicalize() {
                    let root_str = canonical_root.to_string_lossy();
                    if canonical_pattern_dir.starts_with(root_str.as_ref()) {
                        found_root = Some(root.clone());
                        // Extract the relative pattern from the root
                        let relative_dir = canonical_pattern_dir
                            .strip_prefix(root_str.as_ref())
                            .unwrap_or(&canonical_pattern_dir);
                        let relative_dir = relative_dir.strip_prefix('/').unwrap_or(relative_dir);

                        // Combine relative directory with glob suffix
                        relative_pattern = Some(if glob_suffix.is_empty() {
                            relative_dir.to_string()
                        } else if relative_dir.is_empty() {
                            glob_suffix
                        } else {
                            format!("{relative_dir}/{glob_suffix}")
                        });
                        break;
                    }
                }
            }

            match (found_root, relative_pattern) {
                (Some(root), Some(pattern)) => {
                    // Pattern is within an allowed root
                    (vec![root], pattern)
                }
                _ => {
                    // Absolute path is not within any allowed root
                    return Err(ServiceError::Internal(
                        "Path is outside allowed directories".to_string(),
                    ));
                }
            }
        } else {
            // Relative pattern - search in all roots
            (
                self.config.root_directories.clone(),
                validated_pattern.clone(),
            )
        };

        // Collect all potential files first
        let all_files: Vec<(String, u64)> = search_roots
            .iter()
            .flat_map(|root_dir| {
                let root_dir_clone = root_dir.clone();
                let pattern_clone = effective_glob_pattern.clone();
                let glob_set_clone = glob_set.clone();
                WalkDir::new(root_dir)
                    .max_depth(10)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .filter_map(move |entry| {
                        let path = entry.path();
                        let path_str = path.to_string_lossy().to_string();

                        // For relative patterns, check against relative path
                        let matches =
                            if pattern_clone.starts_with("**") || pattern_clone.contains('/') {
                                // Try matching against relative path from root
                                if let Ok(rel_path) = path.strip_prefix(&root_dir_clone) {
                                    glob_set_clone.is_match(rel_path.to_string_lossy().as_ref())
                                } else {
                                    glob_set_clone.is_match(&path_str)
                                }
                            } else {
                                // For simple patterns like "*.js", match against filename
                                if let Some(file_name) = path.file_name() {
                                    glob_set_clone.is_match(file_name.to_string_lossy().as_ref())
                                } else {
                                    false
                                }
                            };

                        if !matches {
                            return None;
                        }

                        // Check file size
                        entry
                            .metadata()
                            .ok()
                            .filter(|m| m.len() <= max_file_size)
                            .map(|m| (path_str, m.len()))
                    })
            })
            .collect();

        // Sort files for consistent pagination
        let mut sorted_files = all_files;
        sorted_files.sort_by(|a, b| a.0.cmp(&b.0));

        // Apply cursor filtering and max_results limit
        let cursor_filter = cursor.as_ref().map(|c| c.last_file_path.clone());
        let mut paginated_files = Vec::new();
        let mut files_processed_count = 0;

        for (path_str, file_size) in sorted_files.into_iter() {
            if let Some(ref start_path) = cursor_filter {
                if path_str.as_str() <= start_path.as_str() {
                    continue;
                }
            }

            if files_processed_count >= max_results {
                // We've reached the limit for this page, set cursor for next page
                let next_cursor = Some(CursorResult {
                    last_file_path: path_str,
                    is_complete: false,
                });
                let files_count = paginated_files.len();
                return Ok((paginated_files, next_cursor, files_count));
            }

            paginated_files.push((path_str, file_size));
            files_processed_count += 1;
        }

        // If we reached here, all matching files have been processed
        let files_count = paginated_files.len();
        Ok((
            paginated_files,
            Some(CursorResult {
                last_file_path: String::new(),
                is_complete: true,
            }),
            files_count,
        ))
    }

    pub async fn search(&self, param: SearchParam) -> Result<SearchResult, ServiceError> {
        let lang = Language::from_str(&param.language)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        // Regular search
        let matches = self.pattern_matcher.search_with_options(
            &param.code,
            &param.pattern,
            lang,
            param.selector.as_deref(),
            param.context.as_deref(),
        )?;

        Ok(SearchResult {
            matches,
            matches_summary: None,
        })
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

        let path_pattern = &param.path_pattern;
        let mut file_results = Vec::new();

        let (file_paths, next_cursor, total_files_found) = self
            .find_and_filter_files(
                path_pattern,
                param.max_file_size,
                param.max_results,
                param.cursor,
            )
            .await?;

        for (file_path, _) in file_paths {
            let content = match std::fs::read_to_string(&file_path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            // Regular search
            let matches = self.pattern_matcher.search_with_options(
                &content,
                &param.pattern,
                lang,
                param.selector.as_deref(),
                param.context.as_deref(),
            )?;

            if !matches.is_empty() {
                file_results.push(FileMatchResult {
                    file_path: file_path.clone(),
                    file_size_bytes: content.len() as u64,
                    matches,
                    file_hash: String::new(),
                });
            }
        }

        Ok(FileSearchResult {
            matches: file_results,
            next_cursor,
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

        let path_pattern = param.path_pattern.as_deref().unwrap_or("**/*");
        let mut file_results = Vec::new();

        let (file_paths, next_cursor, total_files_found) = self
            .find_and_filter_files(
                path_pattern,
                param.max_file_size,
                param.max_results,
                param.cursor,
            )
            .await?;

        for (file_path, _) in file_paths {
            let content = match std::fs::read_to_string(&file_path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            // TODO: Check if file language matches

            let matches = self
                .rule_evaluator
                .evaluate_rule_against_code(&rule.rule, &content, lang)?;

            if !matches.is_empty() {
                file_results.push(FileMatchResult {
                    file_path: file_path.clone(),
                    file_size_bytes: content.len() as u64,
                    matches,
                    file_hash: String::new(),
                });
            }
        }

        Ok(FileSearchResult {
            matches: file_results,
            next_cursor,
            total_files_found,
        })
    }

    fn validate_file_under_roots(&self, file_path: &str) -> Result<(), ServiceError> {
        let path = std::path::Path::new(file_path);
        let canonical_path = path
            .canonicalize()
            .map_err(|e| ServiceError::Internal(format!("Failed to canonicalize path: {e}")))?;

        for root in &self.config.root_directories {
            if let Ok(canonical_root) = root.canonicalize() {
                if canonical_path.starts_with(&canonical_root) {
                    return Ok(());
                }
            }
        }

        Err(ServiceError::Internal(
            "File is outside allowed directories".to_string(),
        ))
    }
}

