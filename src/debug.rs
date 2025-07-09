//! # Debug Service
//!
//! Provides debug functionality for patterns and AST visualization to help users
//! understand how ast-grep processes their patterns and code.
//!
//! ## Features
//!
//! - **Pattern Debugging**: Analyze pattern structure and matching behavior
//! - **AST Visualization**: Generate Abstract Syntax Trees for code analysis
//! - **CST Visualization**: Generate Concrete Syntax Trees with all tokens
//! - **Tree Statistics**: Provide metrics about parsed syntax trees

use crate::errors::ServiceError;
use crate::pattern::PatternMatcher;
use crate::types::*;
use ast_grep_language::SupportLang as Language;
use std::collections::HashSet;
use std::str::FromStr;

/// Service for debugging patterns and AST structures.
#[derive(Clone)]
pub struct DebugService {
    pattern_matcher: PatternMatcher,
}

impl DebugService {
    /// Create a new debug service.
    pub fn new(pattern_matcher: PatternMatcher) -> Self {
        Self { pattern_matcher }
    }

    /// Debug a pattern to understand its structure and behavior.
    pub async fn debug_pattern(
        &self,
        param: DebugPatternParam,
    ) -> Result<DebugPatternResult, ServiceError> {
        let lang = Language::from_str(&param.language)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        let debug_info = match param.format {
            DebugFormat::Pattern => self.debug_pattern_structure(&param.pattern, lang)?,
            DebugFormat::Ast => self.debug_pattern_as_ast(&param.pattern, lang)?,
            DebugFormat::Cst => self.debug_pattern_as_cst(&param.pattern, lang)?,
        };

        let explanation = self.generate_pattern_explanation(&param.pattern, lang);

        // Test pattern against sample code if provided
        let sample_matches = if let Some(ref sample_code) = param.sample_code {
            self.pattern_matcher
                .search_with_options(sample_code, &param.pattern, lang, None, None)
                .ok()
        } else {
            None
        };

        Ok(DebugPatternResult {
            pattern: param.pattern,
            language: param.language,
            format: param.format,
            debug_info,
            sample_matches,
            explanation,
        })
    }

    /// Debug AST/CST structure of code.
    pub async fn debug_ast(&self, param: DebugAstParam) -> Result<DebugAstResult, ServiceError> {
        let lang = Language::from_str(&param.language)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))?;

        let (tree, tree_stats) = match param.format {
            DebugFormat::Ast => self.generate_ast_debug(&param.code, lang)?,
            DebugFormat::Cst => self.generate_cst_debug(&param.code, lang, param.include_trivia)?,
            DebugFormat::Pattern => {
                return Err(ServiceError::Internal(
                    "Pattern format not supported for AST debug".to_string(),
                ));
            }
        };

        let node_kinds = self.extract_node_kinds(&tree);

        Ok(DebugAstResult {
            tree,
            language: param.language,
            format: param.format,
            code_length: param.code.len(),
            node_kinds,
            tree_stats,
        })
    }

    /// Debug pattern structure - show how the pattern is parsed.
    fn debug_pattern_structure(
        &self,
        pattern: &str,
        lang: Language,
    ) -> Result<String, ServiceError> {
        // This would show the internal ast-grep pattern parsing
        // For now, we'll create a helpful breakdown
        let mut debug_info = String::new();
        debug_info.push_str(&format!("Pattern: {pattern}\n"));
        debug_info.push_str(&format!("Language: {lang:?}\n\n"));

        // Analyze pattern components
        debug_info.push_str("Pattern Analysis:\n");
        debug_info.push_str(&self.analyze_pattern_components(pattern));

        // Show metavariables
        let metavars = self.extract_metavariables(pattern);
        if !metavars.is_empty() {
            debug_info.push_str("\nMetavariables found:\n");
            for metavar in metavars {
                debug_info.push_str(&format!("  - {metavar}\n"));
            }
        }

        Ok(debug_info)
    }

    /// Debug pattern as AST - show how the pattern would be parsed as code.
    fn debug_pattern_as_ast(&self, pattern: &str, lang: Language) -> Result<String, ServiceError> {
        // Parse the pattern as if it were regular code
        self.pattern_matcher
            .generate_ast_debug_string(pattern, lang)
            .map_err(|e| ServiceError::Internal(format!("Failed to generate AST: {e}")))
    }

    /// Debug pattern as CST - show complete token structure.
    fn debug_pattern_as_cst(&self, pattern: &str, lang: Language) -> Result<String, ServiceError> {
        // Generate CST representation
        self.pattern_matcher
            .generate_cst_debug_string(pattern, lang)
            .map_err(|e| ServiceError::Internal(format!("Failed to generate CST: {e}")))
    }

    /// Generate AST debug information with statistics.
    fn generate_ast_debug(
        &self,
        code: &str,
        lang: Language,
    ) -> Result<(String, TreeStats), ServiceError> {
        let ast_string = self
            .pattern_matcher
            .generate_ast_debug_string(code, lang)
            .map_err(|e| ServiceError::Internal(format!("Failed to generate AST: {e}")))?;

        let stats = self.calculate_tree_stats(&ast_string, false);
        Ok((ast_string, stats))
    }

    /// Generate CST debug information with statistics.
    fn generate_cst_debug(
        &self,
        code: &str,
        lang: Language,
        include_trivia: bool,
    ) -> Result<(String, TreeStats), ServiceError> {
        let cst_string = if include_trivia {
            self.pattern_matcher
                .generate_cst_debug_string(code, lang)
                .map_err(|e| ServiceError::Internal(format!("Failed to generate CST: {e}")))?
        } else {
            // Generate CST without trivia (similar to AST)
            self.pattern_matcher
                .generate_ast_debug_string(code, lang)
                .map_err(|e| ServiceError::Internal(format!("Failed to generate CST: {e}")))?
        };

        let stats = self.calculate_tree_stats(&cst_string, include_trivia);
        Ok((cst_string, stats))
    }

    /// Extract node kinds from a tree string representation.
    fn extract_node_kinds(&self, tree_string: &str) -> Vec<String> {
        let mut node_kinds = HashSet::new();

        // Parse the tree string to extract node types
        for line in tree_string.lines() {
            if let Some(node_start) = line.find('(') {
                if let Some(node_end) = line[node_start + 1..].find(' ') {
                    let node_kind = &line[node_start + 1..node_start + 1 + node_end];
                    node_kinds.insert(node_kind.to_string());
                } else if let Some(node_end) = line[node_start + 1..].find(')') {
                    let node_kind = &line[node_start + 1..node_start + 1 + node_end];
                    node_kinds.insert(node_kind.to_string());
                }
            }
        }

        let mut result: Vec<String> = node_kinds.into_iter().collect();
        result.sort();
        result
    }

    /// Calculate tree statistics from string representation.
    fn calculate_tree_stats(&self, tree_string: &str, _include_trivia: bool) -> TreeStats {
        let lines: Vec<&str> = tree_string.lines().collect();
        let mut total_nodes = 0;
        let mut leaf_nodes = 0;
        let mut max_depth = 0;
        let mut error_nodes = 0;

        for line in &lines {
            if line.trim().is_empty() {
                continue;
            }

            total_nodes += 1;

            // Calculate depth by counting leading spaces
            let depth = line.len() - line.trim_start().len();
            max_depth = max_depth.max(depth / 2); // Assuming 2 spaces per level

            // Check for leaf nodes (no children)
            if line.contains("\"") || line.ends_with(')') {
                leaf_nodes += 1;
            }

            // Check for error nodes
            if line.contains("ERROR") || line.contains("MISSING") {
                error_nodes += 1;
            }
        }

        TreeStats {
            total_nodes,
            leaf_nodes,
            max_depth,
            error_nodes,
        }
    }

    /// Analyze pattern components and structure.
    fn analyze_pattern_components(&self, pattern: &str) -> String {
        let mut analysis = String::new();

        // Check for metavariables
        if pattern.contains('$') {
            analysis.push_str("  ✓ Contains metavariables for pattern matching\n");
        }

        // Check for wildcards
        if pattern.contains("$_") {
            analysis.push_str("  ✓ Contains anonymous wildcards ($_)\n");
        }

        // Check for ellipsis
        if pattern.contains("$$$") {
            analysis.push_str("  ✓ Contains multi-statement ellipsis ($$$)\n");
        }

        // Check for specific syntax patterns
        if pattern.contains('{') && pattern.contains('}') {
            analysis.push_str("  ✓ Contains block structure\n");
        }

        if pattern.contains('(') && pattern.contains(')') {
            analysis.push_str("  ✓ Contains function call or expression grouping\n");
        }

        if pattern.contains('[') && pattern.contains(']') {
            analysis.push_str("  ✓ Contains array/index access pattern\n");
        }

        if analysis.is_empty() {
            analysis.push_str("  • Simple literal pattern (no metavariables)\n");
        }

        analysis
    }

    /// Extract metavariables from a pattern.
    fn extract_metavariables(&self, pattern: &str) -> Vec<String> {
        let mut metavars = HashSet::new();
        let mut chars = pattern.chars().peekable();

        while let Some(&ch) = chars.peek() {
            if ch == '$' {
                chars.next(); // consume $
                let mut metavar = String::from("$");

                // Handle special cases like $$$ or $_
                if chars.peek() == Some(&'$') {
                    metavar.push(chars.next().unwrap());
                    if chars.peek() == Some(&'$') {
                        metavar.push(chars.next().unwrap());
                    }
                } else if chars.peek() == Some(&'_') {
                    metavar.push(chars.next().unwrap());
                } else {
                    // Regular metavariable - collect alphanumeric characters
                    while let Some(&ch) = chars.peek() {
                        if ch.is_alphanumeric() || ch == '_' {
                            metavar.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                }

                if metavar.len() > 1 {
                    metavars.insert(metavar);
                }
            } else {
                chars.next();
            }
        }

        let mut result: Vec<String> = metavars.into_iter().collect();
        result.sort();
        result
    }

    /// Generate explanation of what a pattern matches.
    fn generate_pattern_explanation(&self, pattern: &str, lang: Language) -> String {
        let mut explanation = String::new();

        explanation.push_str(&format!("This {lang:?} pattern matches:\n"));

        // Analyze structure and provide explanation
        if pattern.contains('$') {
            let metavars = self.extract_metavariables(pattern);
            explanation.push_str("• Code structures where:\n");
            for metavar in metavars {
                match metavar.as_str() {
                    "$$$" => explanation.push_str("  - $$$ matches any sequence of statements\n"),
                    "$_" => {
                        explanation.push_str("  - $_ matches any single expression (anonymous)\n")
                    }
                    _ => explanation
                        .push_str(&format!("  - {metavar} can be any expression/identifier\n")),
                }
            }
        } else {
            explanation.push_str("• Exact literal text matches\n");
        }

        // Add structural hints
        if pattern.contains('{') {
            explanation.push_str("• Block or object structures\n");
        }
        if pattern.contains('(') {
            explanation.push_str("• Function calls or grouped expressions\n");
        }
        if pattern.contains('[') {
            explanation.push_str("• Array access or array literals\n");
        }

        explanation.push_str(
            "\nFor more details, use the AST debug format to see how the pattern is parsed.",
        );

        explanation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::PatternMatcher;

    fn create_debug_service() -> DebugService {
        DebugService::new(PatternMatcher::new())
    }

    #[tokio::test]
    async fn test_debug_pattern_basic() {
        let service = create_debug_service();
        let param = DebugPatternParam {
            pattern: "console.log($ARG)".to_string(),
            language: "javascript".to_string(),
            sample_code: Some("console.log('test')".to_string()),
            format: DebugFormat::Pattern,
        };

        let result = service.debug_pattern(param).await.unwrap();
        assert_eq!(result.pattern, "console.log($ARG)");
        assert_eq!(result.format, DebugFormat::Pattern);
        assert!(result.debug_info.contains("Pattern Analysis"));
        assert!(result.debug_info.contains("$ARG"));
        assert!(result.explanation.contains("matches"));
        assert!(result.sample_matches.is_some());
    }

    #[tokio::test]
    async fn test_debug_ast_basic() {
        let service = create_debug_service();
        let param = DebugAstParam {
            code: "function test() { return 42; }".to_string(),
            language: "javascript".to_string(),
            format: DebugFormat::Ast,
            include_trivia: false,
        };

        let result = service.debug_ast(param).await.unwrap();
        assert_eq!(result.format, DebugFormat::Ast);
        assert!(result.tree.contains("function"));
        assert!(!result.node_kinds.is_empty());
        assert!(result.tree_stats.total_nodes > 0);
    }

    #[tokio::test]
    async fn test_metavar_extraction() {
        let service = create_debug_service();
        let metavars = service.extract_metavariables("$FN($ARG1, $ARG2)");
        assert_eq!(metavars, vec!["$ARG1", "$ARG2", "$FN"]);
    }

    #[tokio::test]
    async fn test_special_metavar_extraction() {
        let service = create_debug_service();
        let metavars = service.extract_metavariables("function $NAME() { $$$ }");
        assert_eq!(metavars, vec!["$$$", "$NAME"]);
    }
}
