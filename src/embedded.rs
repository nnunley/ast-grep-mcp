//! # Embedded Language Service
//!
//! Provides support for searching patterns in embedded languages within host languages.
//! For example, finding JavaScript patterns within HTML script tags, or SQL patterns
//! within Python string literals.

use crate::errors::ServiceError;
use crate::pattern::PatternMatcher;
use crate::types::*;
use ast_grep_core::{AstGrep, tree_sitter::StrDoc};
use ast_grep_language::SupportLang as Language;
use std::str::FromStr;

/// Service for handling embedded language searches.
#[derive(Clone)]
pub struct EmbeddedService {
    pattern_matcher: PatternMatcher,
}

impl EmbeddedService {
    /// Create a new embedded language service.
    pub fn new(pattern_matcher: PatternMatcher) -> Self {
        Self { pattern_matcher }
    }

    /// Search for patterns in embedded languages using native AST access.
    ///
    /// This version demonstrates the power of working with native AST nodes,
    /// allowing us to traverse the tree and extract more contextual information.
    pub async fn search_embedded_native(
        &self,
        param: EmbeddedSearchParam,
    ) -> Result<EmbeddedSearchResult, ServiceError> {
        let host_lang = Language::from_str(&param.embedded_config.host_language)
            .map_err(|_| ServiceError::Internal("Invalid host language".to_string()))?;

        let embedded_lang = Language::from_str(&param.embedded_config.embedded_language)
            .map_err(|_| ServiceError::Internal("Invalid embedded language".to_string()))?;

        // Parse the host language code
        let host_ast = AstGrep::new(&param.code, host_lang);

        // Extract embedded code blocks using native AST
        let embedded_blocks =
            self.extract_embedded_blocks_native(&host_ast, &param.embedded_config, host_lang)?;

        // Search each embedded block
        let mut all_matches = Vec::new();
        for block in &embedded_blocks {
            // Parse the embedded code
            let embedded_ast = AstGrep::new(&block.code, embedded_lang);

            // Search using native AST
            let matches = self.pattern_matcher.search_native(
                &embedded_ast,
                &param.pattern,
                embedded_lang,
                param.embedded_config.selector.as_deref(),
                param.embedded_config.context.as_deref(),
            )?;

            // Convert matches to embedded results with position mapping
            for search_match in matches.iter() {
                let match_result = search_match.to_match_result();

                // Use the native AST information to get better context
                let host_context = if let Some(node) = block.search_match.get_node() {
                    // We can traverse to parent nodes for better context
                    if let Some(parent) = node.parent() {
                        format!(
                            "Block {} in {} at {}:{}",
                            block.index + 1,
                            parent.kind(),
                            block.search_match.start_line(),
                            block.search_match.start_col()
                        )
                    } else {
                        format!(
                            "Block {} at {}:{}",
                            block.index + 1,
                            block.search_match.start_line(),
                            block.search_match.start_col()
                        )
                    }
                } else {
                    format!(
                        "Block {} at {}:{}",
                        block.index + 1,
                        block.search_match.start_line(),
                        block.search_match.start_col()
                    )
                };

                let embedded_match = EmbeddedMatchResult {
                    text: match_result.text,
                    start_line: match_result.start_line + block.search_match.start_line(),
                    start_col: if match_result.start_line == 0 {
                        match_result.start_col + block.search_match.start_col()
                    } else {
                        match_result.start_col
                    },
                    end_line: match_result.end_line + block.search_match.start_line(),
                    end_col: if match_result.end_line == 0 {
                        match_result.end_col + block.search_match.start_col()
                    } else {
                        match_result.end_col
                    },
                    host_context,
                    embedded_block_index: block.index,
                    vars: match_result.vars,
                };
                all_matches.push(embedded_match);
            }
        }

        Ok(EmbeddedSearchResult {
            matches: all_matches,
            host_language: param.embedded_config.host_language,
            embedded_language: param.embedded_config.embedded_language,
            total_embedded_blocks: embedded_blocks.len(),
        })
    }

    /// Search for patterns in embedded languages within code.
    pub async fn search_embedded(
        &self,
        param: EmbeddedSearchParam,
    ) -> Result<EmbeddedSearchResult, ServiceError> {
        let host_lang = Language::from_str(&param.embedded_config.host_language)
            .map_err(|_| ServiceError::Internal("Invalid host language".to_string()))?;

        let embedded_lang = Language::from_str(&param.embedded_config.embedded_language)
            .map_err(|_| ServiceError::Internal("Invalid embedded language".to_string()))?;

        // Extract embedded code blocks from the host language
        let embedded_blocks =
            self.extract_embedded_blocks(&param.code, &param.embedded_config, host_lang)?;

        // Search each embedded block
        let mut all_matches = Vec::new();
        for block in &embedded_blocks {
            let matches = self.pattern_matcher.search_with_options(
                &block.code,
                &param.pattern,
                embedded_lang,
                param.embedded_config.selector.as_deref(),
                param.embedded_config.context.as_deref(),
            )?;

            // Convert matches to embedded results with position mapping
            for match_result in matches {
                let embedded_match = EmbeddedMatchResult {
                    text: match_result.text,
                    start_line: match_result.start_line + block.start_line,
                    start_col: if match_result.start_line == 1 {
                        match_result.start_col + block.start_col
                    } else {
                        match_result.start_col
                    },
                    end_line: match_result.end_line + block.start_line,
                    end_col: if match_result.end_line == 1 {
                        match_result.end_col + block.start_col
                    } else {
                        match_result.end_col
                    },
                    host_context: block.context.clone(),
                    embedded_block_index: block.index,
                    vars: match_result.vars,
                };
                all_matches.push(embedded_match);
            }
        }

        Ok(EmbeddedSearchResult {
            matches: all_matches,
            host_language: param.embedded_config.host_language,
            embedded_language: param.embedded_config.embedded_language,
            total_embedded_blocks: embedded_blocks.len(),
        })
    }

    /// Extract embedded code blocks from host language code.
    fn extract_embedded_blocks(
        &self,
        code: &str,
        config: &EmbeddedLanguageConfig,
        host_lang: Language,
    ) -> Result<Vec<EmbeddedBlock>, ServiceError> {
        // Use the host language to find embedded blocks
        let matches = self.pattern_matcher.search_with_options(
            code,
            &config.extraction_pattern,
            host_lang,
            config.selector.as_deref(),
            config.context.as_deref(),
        )?;

        let mut blocks = Vec::new();
        for (index, match_result) in matches.iter().enumerate() {
            // Extract the actual embedded code from metavariables if present
            let embedded_code = if let Some(code_var) = match_result.vars.get("CODE") {
                code_var.clone()
            } else if let Some(code_var) = match_result.vars.get("JS_CODE") {
                code_var.clone()
            } else {
                match_result.text.clone()
            };

            let block = EmbeddedBlock {
                code: embedded_code,
                start_line: match_result.start_line,
                start_col: match_result.start_col,
                end_line: match_result.end_line,
                end_col: match_result.end_col,
                context: format!(
                    "Block {} at {}:{}",
                    index + 1,
                    match_result.start_line,
                    match_result.start_col
                ),
                index,
            };
            blocks.push(block);
        }

        Ok(blocks)
    }

    /// Extract embedded blocks using native AST traversal
    fn extract_embedded_blocks_native<'a>(
        &self,
        ast: &'a AstGrep<StrDoc<Language>>,
        config: &EmbeddedLanguageConfig,
        host_lang: Language,
    ) -> Result<Vec<EmbeddedBlockNative<'a>>, ServiceError> {
        let matches = self.pattern_matcher.search_native(
            ast,
            &config.extraction_pattern,
            host_lang,
            config.selector.as_deref(),
            config.context.as_deref(),
        )?;

        let mut blocks = Vec::new();
        for (index, search_match) in matches.iter().enumerate() {
            let match_result = search_match.to_match_result();

            // Extract the actual embedded code from metavariables if present
            let embedded_code = if let Some(code_var) = match_result.vars.get("CODE") {
                code_var.clone()
            } else if let Some(code_var) = match_result.vars.get("JS_CODE") {
                code_var.clone()
            } else {
                match_result.text.clone()
            };

            let block = EmbeddedBlockNative {
                code: embedded_code,
                search_match: search_match.clone(),
                index,
            };
            blocks.push(block);
        }

        Ok(blocks)
    }
}

/// Represents an extracted embedded code block.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct EmbeddedBlock {
    /// The extracted code in the embedded language
    code: String,
    /// Starting line in the host file
    start_line: usize,
    /// Starting column in the host file
    start_col: usize,
    /// Ending line in the host file
    end_line: usize,
    /// Ending column in the host file
    end_col: usize,
    /// Context description for this block
    context: String,
    /// Index of this block in the extraction sequence
    index: usize,
}

/// Represents an extracted embedded code block with native AST access.
#[derive(Debug, Clone)]
struct EmbeddedBlockNative<'a> {
    /// The extracted code in the embedded language
    code: String,
    /// The search match with AST access
    search_match: crate::search_match::SearchMatch<'a>,
    /// Index of this block in the extraction sequence
    index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::PatternMatcher;

    fn create_embedded_service() -> EmbeddedService {
        EmbeddedService::new(PatternMatcher::new())
    }

    #[tokio::test]
    async fn test_embedded_search_basic() {
        let service = create_embedded_service();

        let config = EmbeddedLanguageConfig {
            host_language: "html".to_string(),
            embedded_language: "javascript".to_string(),
            extraction_pattern: "<script>$CODE</script>".to_string(),
            selector: None,
            context: None,
        };

        let param = EmbeddedSearchParam {
            code: "<html><script>console.log('test')</script></html>".to_string(),
            pattern: "console.log($ARG)".to_string(),
            embedded_config: config,
            strictness: None,
        };

        let result = service.search_embedded(param).await.unwrap();
        assert_eq!(result.host_language, "html");
        assert_eq!(result.embedded_language, "javascript");
        assert_eq!(result.total_embedded_blocks, 1);
    }
}
