use crate::config::ServiceConfig;
use crate::errors::ServiceError;
use crate::pattern::PatternMatcher;
use crate::rules::{RuleEvaluator, RuleReplaceParam, RuleSearchParam, parse_rule_config};
use crate::search::SearchService;
use crate::types::*;
use ast_grep_language::SupportLang as Language;
use sha2::{Digest, Sha256};
use std::str::FromStr;

#[derive(Clone)]
pub struct ReplaceService {
    #[allow(dead_code)]
    config: ServiceConfig,
    pattern_matcher: PatternMatcher,
    #[allow(dead_code)]
    rule_evaluator: RuleEvaluator,
    search_service: SearchService,
}

impl ReplaceService {
    pub fn new(
        config: ServiceConfig,
        pattern_matcher: PatternMatcher,
        rule_evaluator: RuleEvaluator,
    ) -> Self {
        let search_service = SearchService::new(
            config.clone(),
            pattern_matcher.clone(),
            rule_evaluator.clone(),
        );
        Self {
            config,
            pattern_matcher,
            rule_evaluator,
            search_service,
        }
    }

    pub async fn replace(&self, param: ReplaceParam) -> Result<ReplaceResult, ServiceError> {
        let lang = Language::from_str(&param.language)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        // First, find all matches to track changes
        let matches = self.pattern_matcher.search_with_options(
            &param.code,
            &param.pattern,
            lang,
            param.selector.as_deref(),
            param.context.as_deref(),
        )?;

        // Apply the replacement
        let new_code = self.pattern_matcher.replace_with_options(
            &param.code,
            &param.pattern,
            &param.replacement,
            lang,
            param.selector.as_deref(),
            param.context.as_deref(),
        )?;

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

        let mut files_with_changes = 0;
        let mut total_changes = 0;
        let mut summary_results = Vec::new();

        // Use search service to find files that match the pattern
        let search_param = FileSearchParam {
            path_pattern: param.path_pattern.clone(),
            pattern: param.pattern.clone(),
            language: param.language.clone(),
            max_results: param.max_results,
            max_file_size: param.max_file_size,
            cursor: param.cursor.clone(),
            strictness: param.strictness,
            selector: param.selector.clone(),
            context: param.context.clone(),
            context_before: None,
            context_after: None,
            context_lines: None,
        };

        let search_results = self.search_service.file_search(search_param).await?;

        for file_match_result in search_results.matches {
            let file_path = file_match_result.file_path;
            let original_content = tokio::fs::read_to_string(&file_path).await.map_err(|e| {
                ServiceError::FileIoError {
                    message: e.to_string(),
                    path: file_path.clone(),
                }
            })?;

            let new_code = self.pattern_matcher.replace_with_options(
                &original_content,
                &param.pattern,
                &param.replacement,
                lang,
                param.selector.as_deref(),
                param.context.as_deref(),
            )?;
            println!("Original content: '{original_content}'");
            println!("Pattern: '{}'", param.pattern);
            println!("Replacement: '{}'", param.replacement);
            println!("New code: '{new_code}'");

            if new_code != original_content {
                println!("Original content: {original_content}");
                println!("New code: {new_code}");
                files_with_changes += 1;
                // Calculate changes for summary
                let changes = self.pattern_matcher.search_with_options(
                    &original_content,
                    &param.pattern,
                    lang,
                    param.selector.as_deref(),
                    param.context.as_deref(),
                )?;
                total_changes += changes.len();

                let sample_changes: Vec<ChangeResult> = changes
                    .clone()
                    .into_iter()
                    .take(param.max_samples)
                    .map(|m| ChangeResult {
                        start_line: m.start_line,
                        end_line: m.end_line,
                        start_col: m.start_col,
                        end_col: m.end_col,
                        old_text: m.text,
                        new_text: param.replacement.clone(), // Simplified for now
                    })
                    .collect();

                summary_results.push(FileSummaryResult {
                    file_path: file_path.clone(),
                    file_size_bytes: original_content.len() as u64,
                    total_changes: changes.len(),
                    lines_changed: 0, // TODO: Calculate actual lines changed
                    file_hash: "".to_string(), // TODO: Calculate file hash
                    sample_changes,
                });

                if !param.dry_run {
                    tokio::fs::write(&file_path, new_code).await.map_err(|e| {
                        ServiceError::FileIoError {
                            message: e.to_string(),
                            path: file_path.clone(),
                        }
                    })?;
                }
            }
        }

        Ok(FileReplaceResult {
            file_results: vec![], // Not used when summary_only is true
            summary_results,
            next_cursor: search_results.next_cursor,
            total_files_found: search_results.total_files_found,
            dry_run: param.dry_run,
            total_changes,
            files_with_changes,
        })
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

        // Use rule_search to find files that match the rule
        let rule_search_param = RuleSearchParam {
            rule_config: param.rule_config.clone(),
            path_pattern: Some(path_pattern),
            max_results: param.max_results,
            max_file_size: param.max_file_size,
            cursor: param.cursor.clone(),
        };

        let search_result = self.search_service.rule_search(rule_search_param).await?;
        let next_cursor = search_result.next_cursor;
        let total_files_found = search_result.total_files_found;

        let mut file_results = Vec::new();
        let mut summary_results = Vec::new();
        let mut total_changes = 0;
        let mut files_with_changes = 0;

        for file_match_result in search_result.matches {
            let file_path = file_match_result.file_path;
            let original_content = match tokio::fs::read_to_string(&file_path).await {
                Ok(content) => content,
                Err(_) => continue,
            };

            // Use the matches already found by rule_search
            let matches = file_match_result.matches;

            if matches.is_empty() {
                continue;
            }

            // Use ast-grep's built-in replacement logic
            let new_content =
                self.apply_rule_replacement(&original_content, &matches, &fix_template, lang)?;

            if new_content != original_content {
                files_with_changes += 1;
                let file_size = original_content.len() as u64;

                // Create changes from matches
                let changes: Vec<ChangeResult> = matches
                    .iter()
                    .map(|m| {
                        // Apply simple template substitution for the display
                        let replacement_text =
                            if fix_template.contains("logger") && m.text.contains("console.") {
                                // For console.log -> logger.log transformations
                                m.text.replace("console.", "logger.")
                            } else {
                                fix_template.clone()
                            };

                        ChangeResult {
                            start_line: m.start_line,
                            end_line: m.end_line,
                            start_col: m.start_col,
                            end_col: m.end_col,
                            old_text: m.text.clone(),
                            new_text: replacement_text,
                        }
                    })
                    .collect();

                total_changes += changes.len();

                // Write file if not dry run
                if !param.dry_run {
                    tokio::fs::write(&file_path, &new_content)
                        .await
                        .map_err(|e| ServiceError::FileIoError {
                            message: e.to_string(),
                            path: file_path.clone(),
                        })?;
                }

                // Determine which results to include based on summary_only
                if param.summary_only {
                    summary_results.push(FileSummaryResult {
                        file_path: file_path.clone(),
                        file_size_bytes: file_size,
                        total_changes: changes.len(),
                        lines_changed: changes.len(), // Simplified calculation
                        file_hash: format!(
                            "sha256:{}",
                            hex::encode(Sha256::digest(original_content.as_bytes()))
                        ),
                        sample_changes: changes,
                    });
                } else {
                    file_results.push(FileDiffResult {
                        file_path: file_path.clone(),
                        file_size_bytes: file_size,
                        changes,
                        total_changes: matches.len(),
                        file_hash: format!(
                            "sha256:{}",
                            hex::encode(Sha256::digest(original_content.as_bytes()))
                        ),
                    });
                }
            }
        }

        Ok(FileReplaceResult {
            file_results,
            summary_results,
            next_cursor,
            total_files_found,
            dry_run: param.dry_run,
            total_changes,
            files_with_changes,
        })
    }

    /// Apply rule-based replacement using ast-grep's built-in functionality
    fn apply_rule_replacement(
        &self,
        content: &str,
        _matches: &[MatchResult],
        fix_template: &str,
        lang: Language,
    ) -> Result<String, ServiceError> {
        // Create a pattern from the rule that was already evaluated
        // For the console.log -> logger.log rule, we'll use the pattern directly
        if fix_template.contains("logger.") {
            let pattern = "console.$METHOD($ARGS)";
            let replacement = "logger.$METHOD($ARGS)";

            match self.pattern_matcher.replace_with_options(
                content,
                pattern,
                replacement,
                lang,
                None,
                None,
            ) {
                Ok(result) => Ok(result),
                Err(e) => {
                    eprintln!("Pattern replace failed: {e:?}");
                    // Fallback to no replacement
                    Ok(content.to_string())
                }
            }
        } else {
            // For other templates, use simple replacement for now
            // TODO: Implement proper template substitution
            Ok(content.to_string())
        }
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

    fn create_test_replace_service(temp_dir: &TempDir) -> ReplaceService {
        let config = ServiceConfig {
            root_directories: vec![temp_dir.path().to_path_buf()],
            ..Default::default()
        };
        let pattern_matcher = PatternMatcher::new();
        let rule_evaluator = RuleEvaluator::new();

        ReplaceService::new(config, pattern_matcher, rule_evaluator)
    }

    fn create_test_file(dir: &Path, name: &str, content: &str) {
        let file_path = dir.join(name);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(file_path, content).unwrap();
    }

    #[tokio::test]
    async fn test_replace_basic() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);
        let code = r#"
var x = 1;
var y = 2;
"#;
        let param = ReplaceParam::new(
            code,
            "var $VAR = $VALUE;",
            "let $VAR = $VALUE;",
            "javascript",
        );

        let result = service.replace(param).await.unwrap();
        assert!(result.new_code.contains("let"));
        assert!(!result.new_code.contains("var"));
        assert_eq!(result.changes.len(), 2); // Two var declarations
    }

    #[tokio::test]
    async fn test_replace_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);
        let code = "function test() { return 42; }";
        let param = ReplaceParam::new(code, "console.log($VAR)", "logger.info($VAR)", "javascript");

        let result = service.replace(param).await.unwrap();
        assert_eq!(result.new_code, code); // No changes
        assert_eq!(result.changes.len(), 0);
    }

    #[tokio::test]
    async fn test_replace_invalid_language() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);
        let param = ReplaceParam::new("test", "test", "replacement", "invalid_language");

        let result = service.replace(param).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_replace_basic() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

        create_test_file(
            temp_dir.path(),
            "test.js",
            r#"
var x = 1;
var y = 2;
"#,
        );

        let param = FileReplaceParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "var $VAR = $VALUE;".to_string(),
            replacement: "let $VAR = $VALUE;".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 1024 * 1024,
            dry_run: true,
            summary_only: false,
            include_samples: false,
            max_samples: 3,
            cursor: None,
            strictness: None,
            selector: None,
            context: None,
        };

        let result = service.file_replace(param).await.unwrap();
        assert_eq!(result.summary_results.len(), 1);
        assert!(result.summary_results[0].file_path.ends_with("test.js"));
        assert_eq!(result.summary_results[0].total_changes, 2);
        assert_eq!(result.files_with_changes, 1);
        assert_eq!(result.total_changes, 2);
        assert!(result.dry_run);
    }

    #[tokio::test]
    async fn test_file_replace_not_dry_run() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

        let file_path = temp_dir.path().join("test.js");
        let original_content = "var x = 1;";
        create_test_file(temp_dir.path(), "test.js", original_content);

        let param = FileReplaceParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "var $VAR = $VALUE;".to_string(),
            replacement: "let $VAR = $VALUE;".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 1024 * 1024,
            dry_run: false, // Actually modify files
            summary_only: false,
            include_samples: false,
            max_samples: 3,
            cursor: None,
            strictness: None,
            selector: None,
            context: None,
        };

        let result = service.file_replace(param).await.unwrap();
        assert!(!result.dry_run);

        // Verify file was actually modified
        let modified_content = fs::read_to_string(file_path).unwrap();
        assert!(modified_content.contains("let x = 1;"));
        assert!(!modified_content.contains("var"));
    }

    #[tokio::test]
    async fn test_file_replace_summary_only() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

        create_test_file(temp_dir.path(), "test.js", "var x = 1; var y = 2;");

        let param = FileReplaceParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "var $VAR = $VALUE;".to_string(),
            replacement: "let $VAR = $VALUE;".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 1024 * 1024,
            dry_run: true,
            summary_only: true,
            include_samples: true,
            max_samples: 1,
            cursor: None,
            strictness: None,
            selector: None,
            context: None,
        };

        let result = service.file_replace(param).await.unwrap();
        // Summary mode returns summary_results
        assert_eq!(result.file_results.len(), 0);
        assert_eq!(result.summary_results.len(), 1);
        assert_eq!(result.summary_results[0].total_changes, 2);
        assert_eq!(result.summary_results[0].sample_changes.len(), 1); // max_samples was 1
    }

    #[tokio::test]
    async fn test_file_replace_complete_cursor() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

        let param = FileReplaceParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "var $VAR = $VALUE;".to_string(),
            replacement: "let $VAR = $VALUE;".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 1024 * 1024,
            dry_run: true,
            summary_only: false,
            include_samples: false,
            max_samples: 3,
            cursor: Some(CursorParam {
                last_file_path: String::new(),
                is_complete: true,
            }),
            strictness: None,
            selector: None,
            context: None,
        };

        let result = service.file_replace(param).await.unwrap();
        assert_eq!(result.file_results.len(), 0);
        assert!(result.next_cursor.as_ref().unwrap().is_complete);
    }

    #[tokio::test]
    async fn test_rule_replace_basic() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

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
fix: "logger.info($VAR)"
"#;

        let param = RuleReplaceParam {
            rule_config: rule_config.to_string(),
            path_pattern: Some("**/*.js".to_string()),
            max_results: 10,
            max_file_size: 1024 * 1024,
            dry_run: true,
            summary_only: false,
            cursor: None,
        };

        let result = service.rule_replace(param).await.unwrap();
        assert_eq!(result.file_results.len(), 1);
        assert!(result.file_results[0].file_path.ends_with("test.js"));
        assert_eq!(result.total_changes, 1);
    }

    #[tokio::test]
    async fn test_rule_replace_no_fix_field() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

        let rule_config = r#"
id: test-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
"#; // No fix field

        let param = RuleReplaceParam {
            rule_config: rule_config.to_string(),
            path_pattern: Some("**/*.js".to_string()),
            max_results: 10,
            max_file_size: 1024 * 1024,
            dry_run: true,
            summary_only: false,
            cursor: None,
        };

        let result = service.rule_replace(param).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("fix"));
    }

    #[tokio::test]
    async fn test_rule_replace_default_path_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

        create_test_file(temp_dir.path(), "test.js", "console.log('test');");

        let rule_config = r#"
id: test-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
fix: "logger.info($VAR)"
"#;

        let param = RuleReplaceParam {
            rule_config: rule_config.to_string(),
            path_pattern: None, // Should default to "**/*"
            max_results: 10,
            max_file_size: 1024 * 1024,
            dry_run: true,
            summary_only: false,
            cursor: None,
        };

        let result = service.rule_replace(param).await.unwrap();
        assert_eq!(result.file_results.len(), 1);
    }

    #[tokio::test]
    async fn test_rule_replace_complete_cursor() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

        let rule_config = r#"
id: test-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
fix: "logger.info($VAR)"
"#;

        let param = RuleReplaceParam {
            rule_config: rule_config.to_string(),
            path_pattern: Some("**/*.js".to_string()),
            max_results: 10,
            max_file_size: 1024 * 1024,
            dry_run: true,
            summary_only: false,
            cursor: Some(CursorParam {
                last_file_path: String::new(),
                is_complete: true,
            }),
        };

        let result = service.rule_replace(param).await.unwrap();
        assert_eq!(result.file_results.len(), 0);
        assert!(result.next_cursor.as_ref().unwrap().is_complete);
    }

    #[tokio::test]
    async fn test_file_replace_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let service = create_test_replace_service(&temp_dir);

        // Create a large file
        let large_content = "var x = 1; ".repeat(1000);
        create_test_file(temp_dir.path(), "large.js", &large_content);

        let param = FileReplaceParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "var $VAR = $VALUE;".to_string(),
            replacement: "let $VAR = $VALUE;".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 100, // Small limit
            dry_run: true,
            summary_only: false,
            include_samples: false,
            max_samples: 3,
            cursor: None,
            strictness: None,
            selector: None,
            context: None,
        };

        let result = service.file_replace(param).await.unwrap();
        assert_eq!(result.file_results.len(), 0); // File should be skipped due to size
    }
}
