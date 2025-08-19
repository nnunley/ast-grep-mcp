//! # Validation Engine
//!
//! Validates preconditions and postconditions for refactoring operations.

use super::types::*;
use crate::errors::ServiceError;
use crate::types::FileSearchResult;
use std::collections::HashSet;
use std::str::FromStr;

/// Engine for validating refactoring preconditions
pub struct ValidationEngine {
    /// Reserved keywords per language
    reserved_keywords: std::collections::HashMap<String, HashSet<String>>,
}

impl ValidationEngine {
    /// Create a new validation engine
    pub fn new() -> Self {
        let mut reserved_keywords = std::collections::HashMap::new();
        
        // JavaScript/TypeScript reserved words
        let js_keywords: HashSet<String> = [
            "break", "case", "catch", "class", "const", "continue", "debugger", "default",
            "delete", "do", "else", "export", "extends", "finally", "for", "function",
            "if", "import", "in", "instanceof", "let", "new", "return", "super", "switch",
            "this", "throw", "try", "typeof", "var", "void", "while", "with", "yield",
            "async", "await", "enum", "implements", "interface", "package", "private",
            "protected", "public", "static"
        ].iter().map(|&s| s.to_string()).collect();
        
        reserved_keywords.insert("javascript".to_string(), js_keywords.clone());
        reserved_keywords.insert("typescript".to_string(), js_keywords);

        // Python reserved words
        let py_keywords: HashSet<String> = [
            "False", "None", "True", "and", "as", "assert", "async", "await", "break",
            "class", "continue", "def", "del", "elif", "else", "except", "finally",
            "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal",
            "not", "or", "pass", "raise", "return", "try", "while", "with", "yield"
        ].iter().map(|&s| s.to_string()).collect();
        
        reserved_keywords.insert("python".to_string(), py_keywords);

        // Rust reserved words
        let rust_keywords: HashSet<String> = [
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
            "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
            "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
            "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
            "where", "while"
        ].iter().map(|&s| s.to_string()).collect();
        
        reserved_keywords.insert("rust".to_string(), rust_keywords);

        Self { reserved_keywords }
    }

    /// Check all preconditions for a refactoring
    pub fn check_preconditions(
        &self,
        preconditions: &[Precondition],
        search_results: &FileSearchResult,
        definition: &RefactoringDefinition,
        request: &RefactoringRequest,
    ) -> Result<Vec<String>, ServiceError> {
        let mut warnings = Vec::new();

        for precondition in preconditions {
            match precondition {
                Precondition::NoSideEffectsIn { expression } => {
                    if let Some(warning) = self.check_no_side_effects(expression, search_results)? {
                        warnings.push(warning);
                    }
                }
                Precondition::UniqueName { name } => {
                    if let Some(warning) = self.check_unique_name(
                        name,
                        request,
                        search_results,
                        definition.supported_languages.first().unwrap_or(&"javascript".to_string()),
                    )? {
                        warnings.push(warning);
                    }
                }
                Precondition::ValidScope { pattern } => {
                    if let Some(warning) = self.check_valid_scope(pattern, search_results)? {
                        warnings.push(warning);
                    }
                }
            }
        }

        Ok(warnings)
    }

    /// Check that an expression has no side effects
    fn check_no_side_effects(
        &self,
        _expression: &str,
        search_results: &FileSearchResult,
    ) -> Result<Option<String>, ServiceError> {
        // Check for common side-effect patterns
        let side_effect_patterns = [
            "=",        // Assignment
            "++",       // Increment
            "--",       // Decrement
            "push",     // Array mutation
            "pop",      // Array mutation
            "shift",    // Array mutation
            "unshift",  // Array mutation
            "splice",   // Array mutation
            "delete",   // Object mutation
            "console.", // Console output
            "alert",    // Browser alert
            "fetch",    // Network request
            "axios",    // Network request
            ".post",    // Network request
            ".put",     // Network request
            ".delete",  // Network request
        ];

        for file_match in &search_results.matches {
            for match_result in &file_match.matches {
                let matched_text = &match_result.text;
                for pattern in &side_effect_patterns {
                    if matched_text.contains(pattern) {
                        return Ok(Some(format!(
                            "Expression may have side effects: found '{}' in matched code",
                            pattern
                        )));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Check that a name is unique in the scope
    fn check_unique_name(
        &self,
        _name_pattern: &str,
        request: &RefactoringRequest,
        search_results: &FileSearchResult,
        language: &str,
    ) -> Result<Option<String>, ServiceError> {
        // Get the actual name from request options
        let name = match &request.options {
            Some(options) => {
                options.function_name.as_ref()
                    .or(options.variable_name.as_ref())
                    .or(options.class_name.as_ref())
                    .or(options.new_name.as_ref())
            }
            None => None,
        };

        if let Some(name) = name {
            // Check if it's a reserved keyword
            if let Some(keywords) = self.reserved_keywords.get(language) {
                if keywords.contains(name) {
                    return Ok(Some(format!(
                        "'{}' is a reserved keyword in {}",
                        name, language
                    )));
                }
            }

            // Check if the name already exists in any of the matched files
            // This is a simplified check - a full implementation would do proper scope analysis
            for file_match in &search_results.matches {
                for match_result in &file_match.matches {
                    // Check the context around the match
                    if let Some(ref context_before) = match_result.context_before {
                        for line in context_before {
                            if line.contains(name) {
                                return Ok(Some(format!(
                                    "Name '{}' already exists in the scope",
                                    name
                                )));
                            }
                        }
                    }
                    if let Some(ref context_after) = match_result.context_after {
                        for line in context_after {
                            if line.contains(name) {
                                return Ok(Some(format!(
                                    "Name '{}' already exists in the scope",
                                    name
                                )));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Check that the pattern matches valid scope
    fn check_valid_scope(
        &self,
        _pattern: &str,
        search_results: &FileSearchResult,
    ) -> Result<Option<String>, ServiceError> {
        // Check if all matches are in valid scopes
        // This is a simplified implementation
        for file_match in &search_results.matches {
            if file_match.matches.is_empty() {
                return Ok(Some("Pattern does not match any valid scope".to_string()));
            }
        }

        Ok(None)
    }

    /// Validate a refactoring pattern against test code
    pub fn validate_pattern(
        &self,
        definition: &RefactoringDefinition,
        test_code: &str,
        language: &str,
        custom_pattern: Option<&str>,
    ) -> ValidateRefactoringResponse {
        use ast_grep_core::{AstGrep, Pattern};
        use ast_grep_language::SupportLang as Language;

        let pattern_str = custom_pattern.unwrap_or(&definition.pattern.r#match);
        
        // Parse the test code
        let lang = match Language::from_str(language) {
            Ok(l) => l,
            Err(_) => {
                return ValidateRefactoringResponse {
                    is_valid: false,
                    matches: vec![],
                    errors: Some(vec![format!("Unsupported language: {}", language)]),
                    expected_result: None,
                };
            }
        };

        let ast = AstGrep::new(test_code, lang);
        
        // Try to create the pattern
        let pattern = match Pattern::try_new(pattern_str, lang) {
            Ok(p) => p,
            Err(e) => {
                return ValidateRefactoringResponse {
                    is_valid: false,
                    matches: vec![],
                    errors: Some(vec![format!("Invalid pattern: {}", e)]),
                    expected_result: None,
                };
            }
        };

        // Find matches
        let mut matches = vec![];
        for node_match in ast.root().find_all(pattern) {
            let start_pos = node_match.get_node().start_pos();
            let end_pos = node_match.get_node().end_pos();
            
            matches.push(PatternMatch {
                text: node_match.text().to_string(),
                start: Position {
                    line: start_pos.line(),
                    column: start_pos.column(&node_match),
                },
                end: Position {
                    line: end_pos.line(),
                    column: end_pos.column(&node_match),
                },
                variables: node_match.get_env().clone().into(),
            });
        }

        // Generate expected transformation result
        let expected_result = if !matches.is_empty() {
            // This is simplified - full implementation would apply the actual transformation
            Some(format!("Would transform {} matches", matches.len()))
        } else {
            None
        };

        ValidateRefactoringResponse {
            is_valid: !matches.is_empty(),
            matches,
            errors: None,
            expected_result,
        }
    }
}

impl Default for ValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileMatchResult, MatchResult};

    fn create_test_search_results() -> FileSearchResult {
        FileSearchResult {
            matches: vec![FileMatchResult {
                file_path: "test.js".to_string(),
                file_size_bytes: 100,
                matches: vec![MatchResult {
                    text: "console.log(data)".to_string(),
                    start_line: 1,
                    end_line: 1,
                    start_col: 0,
                    end_col: 17,
                    vars: Default::default(),
                    context_before: Some(vec!["const data = getData();".to_string()]),
                    context_after: Some(vec!["return data;".to_string()]),
                }],
                file_hash: "hash".to_string(),
            }],
            next_cursor: None,
            total_files_found: 1,
        }
    }

    #[test]
    fn test_validation_engine_creation() {
        let engine = ValidationEngine::new();
        assert!(engine.reserved_keywords.contains_key("javascript"));
        assert!(engine.reserved_keywords.contains_key("python"));
        assert!(engine.reserved_keywords.contains_key("rust"));
    }

    #[test]
    fn test_check_no_side_effects() {
        let engine = ValidationEngine::new();
        let mut results = create_test_search_results();
        
        // Test with side effect
        results.matches[0].matches[0].text = "data++".to_string();
        let warning = engine.check_no_side_effects("$EXPR", &results).unwrap();
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("side effects"));

        // Test without side effect
        results.matches[0].matches[0].text = "data + 1".to_string();
        let warning = engine.check_no_side_effects("$EXPR", &results).unwrap();
        assert!(warning.is_none());
    }

    #[test]
    fn test_check_unique_name() {
        let engine = ValidationEngine::new();
        let results = create_test_search_results();
        
        // Test with reserved keyword
        let request = RefactoringRequest {
            refactoring_id: "extract_variable".to_string(),
            pattern_example: None,
            options: Some(RefactoringOptions {
                variable_name: Some("class".to_string()),
                ..Default::default()
            }),
        };
        
        let warning = engine.check_unique_name("$NAME", &request, &results, "javascript").unwrap();
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("reserved keyword"));

        // Test with existing name
        let request = RefactoringRequest {
            refactoring_id: "extract_variable".to_string(),
            pattern_example: None,
            options: Some(RefactoringOptions {
                variable_name: Some("data".to_string()),
                ..Default::default()
            }),
        };
        
        let warning = engine.check_unique_name("$NAME", &request, &results, "javascript").unwrap();
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("already exists"));

        // Test with unique name
        let request = RefactoringRequest {
            refactoring_id: "extract_variable".to_string(),
            pattern_example: None,
            options: Some(RefactoringOptions {
                variable_name: Some("newVariable".to_string()),
                ..Default::default()
            }),
        };
        
        let warning = engine.check_unique_name("$NAME", &request, &results, "javascript").unwrap();
        assert!(warning.is_none());
    }

    #[test]
    fn test_validate_pattern() {
        let engine = ValidationEngine::new();
        let definition = RefactoringDefinition {
            id: "test".to_string(),
            name: "Test".to_string(),
            category: RefactoringCategory::ComposingMethods,
            description: "Test".to_string(),
            supported_languages: vec!["javascript".to_string()],
            complexity: RefactoringComplexity::Simple,
            pattern: PatternDefinition {
                r#match: "console.log($VAR)".to_string(),
                constraints: None,
            },
            transform: TransformDefinition {
                replace: "log($VAR)".to_string(),
                extract: None,
                scope_analysis: None,
                update_calls: None,
            },
            variables: None,
            preconditions: None,
            variants: None,
        };

        let test_code = "console.log('hello'); console.log(data);";
        let response = engine.validate_pattern(&definition, test_code, "javascript", None);
        
        assert!(response.is_valid);
        assert_eq!(response.matches.len(), 2);
        assert!(response.errors.is_none());
    }
}