use crate::config::ServiceConfig;
use crate::errors::ServiceError;
use crate::path_validation::validate_path_pattern;
use crate::pattern::PatternMatcher;
use crate::rules::{RuleEvaluator, RuleSearchParam, ast::Rule, parse_rule_config};
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

        // Validate the path pattern for security
        let validated_pattern = validate_path_pattern(&param.path_pattern)?;

        let glob = Glob::new(&validated_pattern)
            .map_err(|e| ServiceError::Internal(format!("Invalid glob pattern: {e}")))?;
        let mut glob_builder = GlobSetBuilder::new();
        glob_builder.add(glob);
        let glob_set = glob_builder
            .build()
            .map_err(|e| ServiceError::Internal(format!("Failed to build glob set: {e}")))?;

        let glob_set = std::sync::Arc::new(glob_set);

        // Collect all potential files first
        let all_files: Vec<(String, u64)> = self
            .config
            .root_directories
            .iter()
            .flat_map(|root_dir| {
                let root_dir_clone = root_dir.clone();
                let pattern_clone = validated_pattern.clone();
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
            .filter(|(path, _)| cursor_filter.as_ref().is_none_or(|start| path > start))
            .filter_map(|(path_str, file_size)| {
                // Read file and search for matches
                std::fs::read_to_string(&path_str).ok().and_then(|content| {
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
        let last_path = file_results
            .last()
            .map(|r| r.file_path.clone())
            .unwrap_or_default();

        Ok(FileSearchResult {
            matches: file_results,
            next_cursor: Some(CursorResult {
                last_file_path: if is_complete {
                    String::new()
                } else {
                    last_path
                },
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

        // Validate the path pattern for security
        let validated_pattern = validate_path_pattern(&path_pattern)?;

        let mut file_results = Vec::new();
        let mut total_files_found = 0;
        let mut files_processed = 0;

        // Determine starting point for pagination
        let start_after = param.cursor.as_ref().map(|c| c.last_file_path.clone());

        // Determine search roots and pattern
        let (search_roots, glob_pattern) = if validated_pattern.starts_with('/') {
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

        let glob = Glob::new(&glob_pattern)
            .map_err(|e| ServiceError::Internal(format!("Invalid glob pattern: {e}")))?;
        let mut glob_builder = GlobSetBuilder::new();
        glob_builder.add(glob);
        let glob_set = glob_builder
            .build()
            .map_err(|e| ServiceError::Internal(format!("Failed to build glob set: {e}")))?;

        for root_dir in &search_roots {
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

                // Check if path matches the glob pattern
                let matches = if let Ok(rel_path) = path.strip_prefix(root_dir) {
                    glob_set.is_match(rel_path.to_string_lossy().as_ref())
                } else {
                    glob_set.is_match(path_str.as_ref())
                };

                if matches {
                    // Check file size
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.len() > param.max_file_size {
                            continue;
                        }
                    }

                    // Read and search file
                    if let Ok(content) = std::fs::read_to_string(path) {
                        // Convert RuleObject to Rule enum and use new evaluation
                        let rule_enum = Rule::from(rule.rule.clone());
                        let matches = self
                            .rule_evaluator
                            .evaluate_rule(&rule_enum, &content, lang)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServiceConfig;
    use crate::pattern::PatternMatcher;
    use crate::rules::RuleEvaluator;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn create_test_search_service() -> (SearchService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = ServiceConfig {
            root_directories: vec![temp_dir.path().to_path_buf()],
            ..Default::default()
        };
        let pattern_matcher = PatternMatcher::new();
        let rule_evaluator = RuleEvaluator::new();

        (
            SearchService::new(config, pattern_matcher, rule_evaluator),
            temp_dir,
        )
    }

    fn create_test_file(dir: &Path, name: &str, content: &str) {
        let file_path = dir.join(name);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(file_path, content).unwrap();
    }

    #[tokio::test]
    async fn test_search_basic() {
        let (service, _temp_dir) = create_test_search_service();
        let code = r#"
function greet() {
    console.log("Hello");
    console.error("Error");
}
"#;
        let param = SearchParam {
            code: code.to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].text.contains("console.log"));
    }

    #[tokio::test]
    async fn test_search_no_matches() {
        let (service, _temp_dir) = create_test_search_service();
        let code = "function test() { return 42; }";
        let param = SearchParam {
            code: code.to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 0);
    }

    #[tokio::test]
    async fn test_search_invalid_language() {
        let (service, _temp_dir) = create_test_search_service();
        let param = SearchParam {
            code: "test".to_string(),
            pattern: "test".to_string(),
            language: "invalid_language".to_string(),
        };

        let result = service.search(param).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_search_basic() {
        let (service, temp_dir) = create_test_search_service();

        // Create test files
        create_test_file(
            temp_dir.path(),
            "test1.js",
            r#"
function greet() {
    console.log("Hello");
}
"#,
        );
        create_test_file(
            temp_dir.path(),
            "test2.js",
            r#"
function farewell() {
    console.error("Bye");
}
"#,
        );

        let param = FileSearchParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.file_search(param).await.unwrap();
        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].file_path.ends_with("test1.js"));
        assert_eq!(result.matches[0].matches.len(), 1);
    }

    #[tokio::test]
    async fn test_file_search_with_cursor() {
        let (service, temp_dir) = create_test_search_service();

        // Create test files with names that sort predictably
        create_test_file(temp_dir.path(), "a_test.js", "console.log('a');");
        create_test_file(temp_dir.path(), "b_test.js", "console.log('b');");
        create_test_file(temp_dir.path(), "c_test.js", "console.log('c');");

        // First request with limit of 1
        let param = FileSearchParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
            max_results: 1,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result1 = service.file_search(param.clone()).await.unwrap();
        assert_eq!(result1.matches.len(), 1);
        assert!(!result1.next_cursor.as_ref().unwrap().is_complete);

        // Second request using cursor
        let param2 = FileSearchParam {
            cursor: result1.next_cursor.map(|c| CursorParam {
                last_file_path: c.last_file_path,
                is_complete: c.is_complete,
            }),
            ..param
        };

        let result2 = service.file_search(param2).await.unwrap();
        assert_eq!(result2.matches.len(), 1);

        // Verify we got a different file
        assert_ne!(result1.matches[0].file_path, result2.matches[0].file_path);
    }

    #[tokio::test]
    async fn test_file_search_complete_cursor() {
        let (service, _temp_dir) = create_test_search_service();

        let param = FileSearchParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 1024 * 1024,
            cursor: Some(CursorParam {
                last_file_path: String::new(),
                is_complete: true,
            }),
        };

        let result = service.file_search(param).await.unwrap();
        assert_eq!(result.matches.len(), 0);
        assert!(result.next_cursor.as_ref().unwrap().is_complete);
    }

    #[tokio::test]
    async fn test_file_search_size_limit() {
        let (service, temp_dir) = create_test_search_service();

        // Create a large file
        let large_content = "console.log('test');".repeat(1000);
        create_test_file(temp_dir.path(), "large.js", &large_content);

        let param = FileSearchParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 100, // Small limit
            cursor: None,
        };

        let result = service.file_search(param).await.unwrap();
        assert_eq!(result.matches.len(), 0); // File should be skipped due to size
    }

    #[tokio::test]
    async fn test_rule_search_basic() {
        let (service, temp_dir) = create_test_search_service();

        create_test_file(
            temp_dir.path(),
            "test.js",
            r#"
function greet() {
    console.log("Hello");
}
"#,
        );

        let rule_config = r#"
id: test-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
"#;

        let param = RuleSearchParam {
            rule_config: rule_config.to_string(),
            path_pattern: Some("**/*.js".to_string()),
            max_results: 10,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.rule_search(param).await.unwrap();
        assert_eq!(result.matches.len(), 1);
        assert!(result.matches[0].file_path.ends_with("test.js"));
    }

    #[tokio::test]
    async fn test_rule_search_default_path_pattern() {
        let (service, temp_dir) = create_test_search_service();

        create_test_file(temp_dir.path(), "test.js", "console.log('test');");

        let rule_config = r#"
id: test-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
"#;

        let param = RuleSearchParam {
            rule_config: rule_config.to_string(),
            path_pattern: None, // Should default to "**/*"
            max_results: 10,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.rule_search(param).await.unwrap();
        assert_eq!(result.matches.len(), 1);
    }

    #[tokio::test]
    async fn test_rule_search_with_pagination() {
        let (service, temp_dir) = create_test_search_service();

        // Create multiple matching files
        for i in 1..=5 {
            create_test_file(
                temp_dir.path(),
                &format!("test{i}.js"),
                "console.log('test');",
            );
        }

        let rule_config = r#"
id: test-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
"#;

        let param = RuleSearchParam {
            rule_config: rule_config.to_string(),
            path_pattern: Some("**/*.js".to_string()),
            max_results: 2, // Limit to 2 results
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.rule_search(param).await.unwrap();
        assert_eq!(result.matches.len(), 2);
        assert!(!result.next_cursor.as_ref().unwrap().is_complete);
    }

    #[tokio::test]
    async fn test_rule_search_invalid_rule() {
        let (service, _temp_dir) = create_test_search_service();

        let param = RuleSearchParam {
            rule_config: "invalid yaml { content".to_string(),
            path_pattern: Some("**/*.js".to_string()),
            max_results: 10,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.rule_search(param).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rule_search_complete_cursor() {
        let (service, _temp_dir) = create_test_search_service();

        let rule_config = r#"
id: test-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
"#;

        let param = RuleSearchParam {
            rule_config: rule_config.to_string(),
            path_pattern: Some("**/*.js".to_string()),
            max_results: 10,
            max_file_size: 1024 * 1024,
            cursor: Some(CursorParam {
                last_file_path: String::new(),
                is_complete: true,
            }),
        };

        let result = service.rule_search(param).await.unwrap();
        assert_eq!(result.matches.len(), 0);
        assert!(result.next_cursor.as_ref().unwrap().is_complete);
    }
}
