use ast_grep_core::NodeMatch;
use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_language::SupportLang as Language;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Suggest patterns types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestPatternsParam {
    pub code_examples: Vec<String>,
    pub language: String,
    pub max_suggestions: Option<usize>,
    pub specificity_levels: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuggestPatternsResult {
    pub suggestions: Vec<PatternSuggestion>,
    pub language: String,
    pub total_suggestions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSuggestion {
    pub pattern: String,
    pub confidence: f64,
    pub specificity: SpecificityLevel,
    pub explanation: String,
    pub matching_examples: Vec<usize>,
    pub node_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpecificityLevel {
    Exact,
    Specific,
    General,
}

// Basic search and replace types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParam {
    pub code: String,
    pub pattern: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub matches: Vec<MatchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub vars: HashMap<String, String>,
}

impl MatchResult {
    /// Convert a NodeMatch into a MatchResult, extracting position and variable information
    pub fn from_node_match(node: &NodeMatch<StrDoc<Language>>) -> Self {
        let vars: HashMap<String, String> = node.get_env().clone().into();
        let start_pos = node.get_node().start_pos();
        let end_pos = node.get_node().end_pos();

        MatchResult {
            text: node.text().to_string(),
            start_line: start_pos.line(),
            end_line: end_pos.line(),
            start_col: start_pos.column(node),
            end_col: end_pos.column(node),
            vars,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchParam {
    pub path_pattern: String,
    pub pattern: String,
    pub language: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    pub cursor: Option<CursorParam>,
}

impl Default for FileSearchParam {
    fn default() -> Self {
        Self {
            path_pattern: "**/*".to_string(),
            pattern: String::new(),
            language: String::new(),
            max_results: default_max_results(),
            max_file_size: default_max_file_size(),
            cursor: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorParam {
    pub last_file_path: String,
    pub is_complete: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSearchResult {
    pub matches: Vec<FileMatchResult>,
    pub next_cursor: Option<CursorResult>,
    pub total_files_found: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatchResult {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub matches: Vec<MatchResult>,
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorResult {
    pub last_file_path: String,
    pub is_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceParam {
    pub code: String,
    pub pattern: String,
    pub replacement: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplaceResult {
    pub new_code: String,
    pub changes: Vec<ChangeResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeResult {
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReplaceParam {
    pub path_pattern: String,
    pub pattern: String,
    pub replacement: String,
    pub language: String,
    #[serde(default = "default_max_results_large")]
    pub max_results: usize,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default = "default_false")]
    pub summary_only: bool,
    #[serde(default = "default_false")]
    pub include_samples: bool,
    #[serde(default = "default_max_samples")]
    pub max_samples: usize,
    pub cursor: Option<CursorParam>,
}

impl Default for FileReplaceParam {
    fn default() -> Self {
        Self {
            path_pattern: "**/*".to_string(),
            pattern: String::new(),
            replacement: String::new(),
            language: String::new(),
            max_results: default_max_results_large(),
            max_file_size: default_max_file_size(),
            dry_run: default_true(),
            summary_only: default_false(),
            include_samples: default_false(),
            max_samples: default_max_samples(),
            cursor: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileReplaceResult {
    pub file_results: Vec<FileDiffResult>,
    pub summary_results: Vec<FileSummaryResult>,
    pub next_cursor: Option<CursorResult>,
    pub total_files_found: usize,
    pub dry_run: bool,
    pub total_changes: usize,
    pub files_with_changes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileDiffResult {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub changes: Vec<ChangeResult>,
    pub total_changes: usize,
    pub file_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileDiffChange {
    pub line_number: usize,
    pub old_content: String,
    pub new_content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSummaryResult {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub total_changes: usize,
    pub lines_changed: usize,
    pub file_hash: String,
    pub sample_changes: Vec<ChangeResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListLanguagesParam {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListLanguagesResult {
    pub languages: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationParam {}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationResult {
    pub content: String,
}

// Catalog types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCatalogRulesParam {
    pub language: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCatalogRulesResult {
    pub rules: Vec<CatalogRuleInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub category: String,
    pub url: String,
}

// AST generation types
#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateAstParam {
    pub code: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateAstResult {
    pub ast: String,
    pub language: String,
    pub code_length: usize,
    pub node_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCatalogRuleParam {
    pub rule_url: String,
    pub rule_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCatalogRuleResult {
    pub rule_id: String,
    pub imported: bool,
    pub message: String,
}

// Default functions for serde
fn default_max_results() -> usize {
    20
}
fn default_max_results_large() -> usize {
    10000
}
fn default_max_file_size() -> u64 {
    50 * 1024 * 1024
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_max_samples() -> usize {
    3
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast_grep_core::{AstGrep, Pattern};
    use ast_grep_language::SupportLang as Language;

    #[test]
    fn test_match_result_from_node_match() {
        let code = "console.log('test');";
        let lang = Language::JavaScript;
        let ast = AstGrep::new(code, lang);
        let pattern = Pattern::new("console.log($VAR)", lang);

        if let Some(node_match) = ast.root().find(pattern) {
            let match_result = MatchResult::from_node_match(&node_match);

            assert!(match_result.text.contains("console.log"));
            assert_eq!(match_result.start_line, 0); // ast-grep uses 0-based line indexing
            assert_eq!(match_result.start_col, 0);
            // Variables should contain VAR
            assert!(match_result.vars.contains_key("VAR"));
        } else {
            panic!("Pattern should match");
        }
    }

    #[test]
    fn test_file_search_param_default() {
        let param = FileSearchParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
            ..Default::default()
        };

        assert_eq!(param.max_results, 20);
        assert_eq!(param.max_file_size, 50 * 1024 * 1024);
        assert!(param.cursor.is_none());
    }

    #[test]
    fn test_file_replace_param_default() {
        let param = FileReplaceParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "var $VAR = $VALUE;".to_string(),
            replacement: "let $VAR = $VALUE;".to_string(),
            language: "javascript".to_string(),
            ..Default::default()
        };

        assert_eq!(param.max_results, 10000);
        assert_eq!(param.max_file_size, 50 * 1024 * 1024);
        assert!(param.dry_run); // Should default to true
        assert!(!param.summary_only); // Should default to false
        assert!(!param.include_samples); // Should default to false
        assert_eq!(param.max_samples, 3);
        assert!(param.cursor.is_none());
    }

    #[test]
    fn test_search_param_serialization() {
        let param = SearchParam {
            code: "console.log('test');".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
        };

        let serialized = serde_json::to_string(&param).unwrap();
        let deserialized: SearchParam = serde_json::from_str(&serialized).unwrap();

        assert_eq!(param.code, deserialized.code);
        assert_eq!(param.pattern, deserialized.pattern);
        assert_eq!(param.language, deserialized.language);
    }

    #[test]
    fn test_replace_param_serialization() {
        let param = ReplaceParam {
            code: "var x = 1;".to_string(),
            pattern: "var $VAR = $VALUE;".to_string(),
            replacement: "let $VAR = $VALUE;".to_string(),
            language: "javascript".to_string(),
        };

        let serialized = serde_json::to_string(&param).unwrap();
        let deserialized: ReplaceParam = serde_json::from_str(&serialized).unwrap();

        assert_eq!(param.code, deserialized.code);
        assert_eq!(param.pattern, deserialized.pattern);
        assert_eq!(param.replacement, deserialized.replacement);
        assert_eq!(param.language, deserialized.language);
    }

    #[test]
    fn test_cursor_param_serialization() {
        let cursor = CursorParam {
            last_file_path: "test/file.js".to_string(),
            is_complete: false,
        };

        let serialized = serde_json::to_string(&cursor).unwrap();
        let deserialized: CursorParam = serde_json::from_str(&serialized).unwrap();

        assert_eq!(cursor.last_file_path, deserialized.last_file_path);
        assert_eq!(cursor.is_complete, deserialized.is_complete);
    }

    #[test]
    fn test_change_result_creation() {
        let change = ChangeResult {
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 10,
            old_text: "var x = 1;".to_string(),
            new_text: "let x = 1;".to_string(),
        };

        assert_eq!(change.start_line, 1);
        assert_eq!(change.old_text, "var x = 1;");
        assert_eq!(change.new_text, "let x = 1;");
    }

    #[test]
    fn test_file_diff_result_creation() {
        let changes = vec![
            ChangeResult {
                start_line: 1,
                end_line: 1,
                start_col: 0,
                end_col: 10,
                old_text: "var x = 1;".to_string(),
                new_text: "let x = 1;".to_string(),
            },
            ChangeResult {
                start_line: 2,
                end_line: 2,
                start_col: 0,
                end_col: 10,
                old_text: "var y = 2;".to_string(),
                new_text: "let y = 2;".to_string(),
            },
        ];

        let diff_result = FileDiffResult {
            file_path: "test.js".to_string(),
            file_size_bytes: 1024,
            changes: changes.clone(),
            total_changes: changes.len(),
            file_hash: "abc123".to_string(),
        };

        assert_eq!(diff_result.file_path, "test.js");
        assert_eq!(diff_result.total_changes, 2);
        assert_eq!(diff_result.changes.len(), 2);
    }

    #[test]
    fn test_file_summary_result_creation() {
        let sample_changes = vec![ChangeResult {
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 10,
            old_text: "var x = 1;".to_string(),
            new_text: "let x = 1;".to_string(),
        }];

        let summary_result = FileSummaryResult {
            file_path: "test.js".to_string(),
            file_size_bytes: 1024,
            total_changes: 5,
            lines_changed: 3,
            file_hash: "abc123".to_string(),
            sample_changes,
        };

        assert_eq!(summary_result.file_path, "test.js");
        assert_eq!(summary_result.total_changes, 5);
        assert_eq!(summary_result.lines_changed, 3);
        assert_eq!(summary_result.sample_changes.len(), 1);
    }

    #[test]
    fn test_generate_ast_param_serialization() {
        let param = GenerateAstParam {
            code: "function test() {}".to_string(),
            language: "javascript".to_string(),
        };

        let serialized = serde_json::to_string(&param).unwrap();
        let deserialized: GenerateAstParam = serde_json::from_str(&serialized).unwrap();

        assert_eq!(param.code, deserialized.code);
        assert_eq!(param.language, deserialized.language);
    }

    #[test]
    fn test_list_catalog_rules_param_optional_fields() {
        let param1 = ListCatalogRulesParam {
            language: Some("javascript".to_string()),
            category: None,
        };

        let param2 = ListCatalogRulesParam {
            language: None,
            category: Some("best-practices".to_string()),
        };

        assert!(param1.language.is_some());
        assert!(param1.category.is_none());
        assert!(param2.language.is_none());
        assert!(param2.category.is_some());
    }

    #[test]
    fn test_import_catalog_rule_result() {
        let result = ImportCatalogRuleResult {
            rule_id: "test-rule".to_string(),
            imported: true,
            message: "Successfully imported".to_string(),
        };

        assert_eq!(result.rule_id, "test-rule");
        assert!(result.imported);
        assert!(result.message.contains("Successfully"));
    }

    #[test]
    fn test_default_functions() {
        assert_eq!(default_max_results(), 20);
        assert_eq!(default_max_results_large(), 10000);
        assert_eq!(default_max_file_size(), 50 * 1024 * 1024);
        assert!(default_true());
        assert!(!default_false());
        assert_eq!(default_max_samples(), 3);
    }
}
