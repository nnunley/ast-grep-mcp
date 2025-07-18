//! # AST Utilities Module
//!
//! Provides utilities for working with AST parsing and pattern creation,
//! including caching for improved performance.

use ast_grep_core::{AstGrep, Pattern, tree_sitter::StrDoc};
use ast_grep_language::SupportLang as Language;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::errors::ServiceError;

/// Utilities for working with AST parsing and pattern creation
#[derive(Clone)]
pub struct AstParser {
    pattern_cache: Arc<Mutex<HashMap<String, Pattern>>>,
}

impl Default for AstParser {
    fn default() -> Self {
        Self {
            pattern_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl AstParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new AST from code
    pub fn parse_code(&self, code: &str, lang: Language) -> AstGrep<StrDoc<Language>> {
        AstGrep::new(code, lang)
    }

    /// Get or create a pattern with caching
    pub fn get_or_create_pattern(
        &self,
        pattern_str: &str,
        lang: Language,
    ) -> Result<Pattern, ServiceError> {
        let cache_key = format!("{lang}:{pattern_str}");

        // Try to get from cache first
        {
            let cache = self.pattern_cache.lock().unwrap();
            if let Some(pattern) = cache.get(&cache_key) {
                return Ok(pattern.clone());
            }
        }

        // Create new pattern
        let pattern = Pattern::new(pattern_str, lang);

        // Store in cache
        {
            let mut cache = self.pattern_cache.lock().unwrap();
            cache.insert(cache_key, pattern.clone());
        }

        Ok(pattern)
    }

    /// Build a string representation of the AST
    pub fn build_ast_string<D: ast_grep_core::Doc>(
        node: ast_grep_core::Node<D>,
        depth: usize,
    ) -> String {
        let indent = "  ".repeat(depth);
        let mut result = format!(
            "{}{}[{}:{}]",
            indent,
            node.kind(),
            node.range().start,
            node.range().end
        );

        // Add node text if it's a leaf node or short
        let node_text = node.text();
        if node.children().count() == 0 || node_text.len() <= 50 {
            let escaped_text = node_text.replace('\n', "\\n").replace('\r', "\\r");
            if !escaped_text.trim().is_empty() {
                result.push_str(&format!(" \"{escaped_text}\""));
            }
        }

        result.push('\n');

        // Recursively add children
        for child in node.children() {
            result.push_str(&Self::build_ast_string(child, depth + 1));
        }

        result
    }

    /// Generate a stringified AST for debugging
    pub fn generate_ast_debug_string(&self, code: &str, lang: Language) -> String {
        let ast = self.parse_code(code, lang);
        Self::build_ast_string(ast.root(), 0)
    }
}

/// Builder for creating patterns with various options
pub struct PatternBuilder {
    pattern: String,
    context: Option<String>,
    selector: Option<String>,
    strictness: Option<String>,
}

impl PatternBuilder {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            context: None,
            selector: None,
            strictness: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_selector(mut self, selector: impl Into<String>) -> Self {
        self.selector = Some(selector.into());
        self
    }

    pub fn with_strictness(mut self, strictness: impl Into<String>) -> Self {
        self.strictness = Some(strictness.into());
        self
    }

    pub fn build(self) -> String {
        // For now, just return the pattern
        // In a real implementation, this would construct the proper pattern based on options
        self.pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_caching() {
        let parser = AstParser::new();
        let lang = Language::JavaScript;
        let pattern = "console.log($VAR)";

        // First call should create and cache
        let result1 = parser.get_or_create_pattern(pattern, lang).unwrap();

        // Second call should return from cache
        let result2 = parser.get_or_create_pattern(pattern, lang).unwrap();

        // Both should be equal (same pattern)
        assert_eq!(format!("{result1:?}"), format!("{result2:?}"));
    }

    #[test]
    fn test_pattern_builder() {
        let pattern = PatternBuilder::new("$VAR = $VALUE")
            .with_context("function $FUNC() { $$$ }")
            .with_selector("assignment_expression")
            .build();

        assert_eq!(pattern, "$VAR = $VALUE");
    }
}
