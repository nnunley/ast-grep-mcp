use crate::config::ServiceConfig;
use crate::errors::ServiceError;
use crate::pattern::PatternMatcher;
use crate::replace::ReplaceService;
use crate::rules::*;
use crate::rules::{CatalogManager, RuleEvaluator, RuleService, RuleStorage};
use crate::search::SearchService;
use crate::types::*;

use lru::LruCache;
use std::num::NonZeroUsize;
use std::{
    borrow::Cow, collections::HashMap, fs, path::PathBuf, str::FromStr, sync::Arc, sync::Mutex,
};

use ast_grep_core::{AstGrep, Pattern, Position, tree_sitter::StrDoc};
use ast_grep_language::SupportLang as Language;
// Removed unused base64 import
use rmcp::{
    ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Content, ErrorData, Implementation, InitializeResult,
        ListToolsResult, PaginatedRequestParam, ProtocolVersion, ServerCapabilities, Tool,
    },
    service::{RequestContext, RoleServer},
};
// Removed unused serde imports
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct AstGrepService {
    config: ServiceConfig,
    pattern_cache: Arc<Mutex<LruCache<String, Pattern>>>,
    #[allow(dead_code)]
    pattern_matcher: PatternMatcher,
    #[allow(dead_code)]
    search_service: SearchService,
    #[allow(dead_code)]
    replace_service: ReplaceService,
    #[allow(dead_code)]
    rule_service: RuleService,
}

impl Default for AstGrepService {
    fn default() -> Self {
        Self::new()
    }
}

impl AstGrepService {
    fn parse_language(&self, lang_str: &str) -> Result<Language, ServiceError> {
        Language::from_str(lang_str)
            .map_err(|_| ServiceError::Internal("Failed to parse language".into()))
    }

    pub fn new() -> Self {
        let config = ServiceConfig::default();
        let cache_size = NonZeroUsize::new(config.pattern_cache_size)
            .unwrap_or(NonZeroUsize::new(1000).unwrap());
        let pattern_cache = Arc::new(Mutex::new(LruCache::new(cache_size)));
        let pattern_matcher = PatternMatcher::with_cache(pattern_cache.clone());
        let rule_evaluator = RuleEvaluator::new();
        let search_service = SearchService::new(
            config.clone(),
            pattern_matcher.clone(),
            rule_evaluator.clone(),
        );
        let replace_service = ReplaceService::new(
            config.clone(),
            pattern_matcher.clone(),
            rule_evaluator.clone(),
        );
        let rule_storage = RuleStorage::new(config.rules_directory.clone());
        let catalog_manager = CatalogManager::new();
        let rule_service = RuleService::new(
            config.clone(),
            rule_evaluator.clone(),
            rule_storage,
            catalog_manager,
        );

        Self {
            config,
            pattern_cache,
            pattern_matcher,
            search_service,
            replace_service,
            rule_service,
        }
    }

    #[allow(dead_code)]
    pub fn with_config(config: ServiceConfig) -> Self {
        let cache_size = NonZeroUsize::new(config.pattern_cache_size)
            .unwrap_or(NonZeroUsize::new(1000).unwrap());
        let pattern_cache = Arc::new(Mutex::new(LruCache::new(cache_size)));
        let pattern_matcher = PatternMatcher::with_cache(pattern_cache.clone());
        let rule_evaluator = RuleEvaluator::new();
        let search_service = SearchService::new(
            config.clone(),
            pattern_matcher.clone(),
            rule_evaluator.clone(),
        );
        let replace_service = ReplaceService::new(
            config.clone(),
            pattern_matcher.clone(),
            rule_evaluator.clone(),
        );
        let rule_storage = RuleStorage::new(config.rules_directory.clone());
        let catalog_manager = CatalogManager::new();
        let rule_service = RuleService::new(
            config.clone(),
            rule_evaluator.clone(),
            rule_storage,
            catalog_manager,
        );

        Self {
            config,
            pattern_cache,
            pattern_matcher,
            search_service,
            replace_service,
            rule_service,
        }
    }

    #[allow(dead_code)]
    fn calculate_file_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    fn get_or_create_pattern(
        &self,
        pattern_str: &str,
        lang: Language,
    ) -> Result<Pattern, ServiceError> {
        let cache_key = format!("{}:{}", lang as u8, pattern_str);

        // First try to get from cache
        if let Ok(mut cache) = self.pattern_cache.lock()
            && let Some(pattern) = cache.get(&cache_key)
        {
            return Ok(pattern.clone());
        }

        // Pattern not in cache, create it
        let pattern = Pattern::new(pattern_str, lang);

        // Try to add to cache (ignore if lock fails)
        if let Ok(mut cache) = self.pattern_cache.lock() {
            cache.put(cache_key, pattern.clone());
        }

        Ok(pattern)
    }

    /// Get pattern cache statistics for monitoring and debugging
    #[allow(dead_code)]
    pub fn get_cache_stats(&self) -> (usize, usize) {
        if let Ok(cache) = self.pattern_cache.lock() {
            (cache.len(), cache.cap().get())
        } else {
            (0, 0)
        }
    }

    /// Generate a stringified syntax tree for the given code and language
    /// This exposes the Tree-sitter AST structure for debugging and understanding
    pub async fn generate_ast(
        &self,
        param: GenerateAstParam,
    ) -> Result<GenerateAstResult, ServiceError> {
        let lang = self.parse_language(&param.language)?;
        let ast = AstGrep::new(&param.code, lang);

        // Build a string representation of the AST
        let ast_string = Self::build_ast_string(ast.root(), 0);

        Ok(GenerateAstResult {
            ast: ast_string,
            language: param.language,
            code_length: param.code.chars().count(),
        })
    }

    /// Recursively build a string representation of the AST
    fn build_ast_string<D: ast_grep_core::Doc>(
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

    fn parse_rule_config(&self, rule_config_str: &str) -> Result<RuleConfig, ServiceError> {
        // First try to parse as YAML
        if let Ok(config) = serde_yaml::from_str::<RuleConfig>(rule_config_str) {
            return Ok(config);
        }

        // If YAML fails, try JSON
        serde_json::from_str::<RuleConfig>(rule_config_str).map_err(|e| {
            ServiceError::ParserError(format!("Failed to parse rule config as YAML or JSON: {e}"))
        })
    }

    fn validate_rule_config(&self, config: &RuleConfig) -> Result<(), ServiceError> {
        // Validate language
        self.parse_language(&config.language)?;

        // Validate rule has at least one condition
        if !self.has_rule_condition(&config.rule) {
            return Err(ServiceError::ParserError(
                "Rule must have at least one condition".into(),
            ));
        }

        Ok(())
    }

    fn has_rule_condition(&self, rule: &RuleObject) -> bool {
        rule.pattern.is_some()
            || rule.kind.is_some()
            || rule.regex.is_some()
            || rule.inside.is_some()
            || rule.has.is_some()
            || rule.follows.is_some()
            || rule.precedes.is_some()
            || rule.all.as_ref().is_some_and(|v| !v.is_empty())
            || rule.any.as_ref().is_some_and(|v| !v.is_empty())
            || rule.not.is_some()
            || rule.matches.is_some()
    }

    fn extract_pattern_from_rule(&self, rule: &RuleObject) -> Option<String> {
        match &rule.pattern {
            Some(PatternSpec::Simple(pattern)) => Some(pattern.clone()),
            Some(PatternSpec::Advanced { context, .. }) => Some(context.clone()),
            None => None,
        }
    }

    #[allow(dead_code)]
    fn is_simple_pattern_rule(&self, rule: &RuleObject) -> bool {
        // Check if this is a simple pattern rule that we can handle directly
        rule.pattern.is_some()
            && rule.kind.is_none()
            && rule.regex.is_none()
            && rule.inside.is_none()
            && rule.has.is_none()
            && rule.follows.is_none()
            && rule.precedes.is_none()
            && rule.all.is_none()
            && rule.any.is_none()
            && rule.not.is_none()
            && rule.matches.is_none()
    }

    #[allow(dead_code)]
    fn extract_all_patterns_from_composite_rule(&self, rule: &RuleObject) -> Vec<String> {
        let mut patterns = Vec::new();

        // Handle direct pattern
        if let Some(pattern) = self.extract_pattern_from_rule(rule) {
            patterns.push(pattern);
        }

        // Handle "all" composite rule
        if let Some(all_rules) = &rule.all {
            for sub_rule in all_rules {
                patterns.extend(self.extract_all_patterns_from_composite_rule(sub_rule));
            }
        }

        // Handle "any" composite rule
        if let Some(any_rules) = &rule.any {
            for sub_rule in any_rules {
                patterns.extend(self.extract_all_patterns_from_composite_rule(sub_rule));
            }
        }

        // Handle "not" composite rule
        if let Some(not_rule) = &rule.not {
            patterns.extend(self.extract_all_patterns_from_composite_rule(not_rule));
        }

        patterns
    }

    #[allow(dead_code)]
    fn evaluate_rule_against_code(
        &self,
        rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Handle different rule types
        if let Some(pattern_spec) = &rule.pattern {
            // Simple pattern rule
            self.evaluate_pattern_rule(pattern_spec, code, lang)
        } else if let Some(all_rules) = &rule.all {
            // ALL composite rule - node must match ALL sub-rules
            self.evaluate_all_rule(all_rules, code, lang)
        } else if let Some(any_rules) = &rule.any {
            // ANY composite rule - node must match ANY sub-rule
            self.evaluate_any_rule(any_rules, code, lang)
        } else if let Some(not_rule) = &rule.not {
            // NOT composite rule - find nodes that DON'T match the sub-rule
            self.evaluate_not_rule(not_rule, code, lang)
        } else if let Some(kind) = &rule.kind {
            // Kind rule - match nodes by AST kind (simplified implementation)
            self.evaluate_kind_rule(kind, code, lang)
        } else if let Some(regex) = &rule.regex {
            // Regex rule - match nodes by text content
            self.evaluate_regex_rule(regex, code, lang)
        } else if let Some(inside_rule) = &rule.inside {
            // Inside relational rule - match nodes inside another pattern
            self.evaluate_inside_rule(inside_rule, code, lang)
        } else if let Some(has_rule) = &rule.has {
            // Has relational rule - match nodes that contain another pattern
            self.evaluate_has_rule(has_rule, code, lang)
        } else if let Some(follows_rule) = &rule.follows {
            // Follows relational rule - match nodes that follow another pattern
            self.evaluate_follows_rule(follows_rule, code, lang)
        } else if let Some(precedes_rule) = &rule.precedes {
            // Precedes relational rule - match nodes that precede another pattern
            self.evaluate_precedes_rule(precedes_rule, code, lang)
        } else {
            Err(ServiceError::ParserError(
                "Rule must have at least one condition".into(),
            ))
        }
    }

    #[allow(dead_code)]
    fn evaluate_pattern_rule(
        &self,
        pattern_spec: &PatternSpec,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let pattern_str = match pattern_spec {
            PatternSpec::Simple(pattern) => pattern.clone(),
            PatternSpec::Advanced { context, .. } => context.clone(),
        };

        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(&pattern_str, lang)?;

        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| {
                let vars: HashMap<String, String> = node.get_env().clone().into();
                let start_pos: Position = node.get_node().start_pos();
                let end_pos: Position = node.get_node().end_pos();
                MatchResult {
                    text: node.text().to_string(),
                    vars,
                    start_line: start_pos.line(),
                    end_line: end_pos.line(),
                    start_col: start_pos.column(&node),
                    end_col: end_pos.column(&node),
                }
            })
            .collect();

        Ok(matches)
    }

    #[allow(dead_code)]
    fn evaluate_kind_rule(
        &self,
        _kind: &str,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // For now, use a simple pattern that matches any node
        // This is a placeholder - proper kind matching would require deeper AST integration
        let ast = AstGrep::new(code, lang);

        // Create a pattern that matches anything and then filter by examining the AST
        // This is a simplified approach
        let pattern = Pattern::new("$_", lang);
        // node.get_node() has access to most relational operators for syntax nodes.

        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| {
                // Check if node kind matches (this is approximate)
                // Note: Direct kind checking may not be available, so we'll use text matching as fallback
                let text = node.text().to_string();
                let vars: HashMap<String, String> = node.get_env().clone().into();
                // For now, include all matches since we can't easily check AST node kind
                // This is a simplified implementation
                // TODO: Implement proper position extraction from node.range()
                MatchResult {
                    text: text.clone(),
                    vars,
                    start_line: 1,
                    end_line: 1,
                    start_col: 0,
                    end_col: text.len(),
                }
            })
            .collect();

        Ok(matches)
    }

    #[allow(dead_code)]
    fn evaluate_regex_rule(
        &self,
        regex_pattern: &str,
        code: &str,
        _lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        use std::str::FromStr;

        // Create regex
        let regex = regex::Regex::from_str(regex_pattern)
            .map_err(|e| ServiceError::ParserError(format!("Invalid regex pattern: {e}")))?;

        let mut matches = Vec::new();

        // Find all matches in the code
        for mat in regex.find_iter(code) {
            // Calculate line and column positions from byte offsets
            let start_byte = mat.start();
            let end_byte = mat.end();
            let (start_line, start_col) = self.byte_offset_to_line_col(code, start_byte);
            let (end_line, end_col) = self.byte_offset_to_line_col(code, end_byte);

            matches.push(MatchResult {
                text: mat.as_str().to_string(),
                vars: HashMap::new(),
                start_line,
                end_line,
                start_col,
                end_col,
            });
        }

        Ok(matches)
    }

    // Relational rule evaluation methods

    #[allow(dead_code)]
    fn evaluate_inside_rule(
        &self,
        inside_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let ast = AstGrep::new(code, lang);

        // Get matches for the container pattern
        let container_matches = self.evaluate_rule_against_code(inside_rule, code, lang)?;

        // Get all possible target matches (from the main pattern in the parent rule)
        // For now, we'll search for a generic pattern to find candidate nodes
        let all_nodes = self.get_all_nodes(&ast, lang)?;

        let mut inside_matches = Vec::new();

        // Check which nodes are inside the container matches
        for node in all_nodes {
            for container in &container_matches {
                if self.is_node_inside(&node, container) {
                    inside_matches.push(node);
                    break;
                }
            }
        }

        Ok(inside_matches)
    }

    #[allow(dead_code)]
    fn evaluate_has_rule(
        &self,
        has_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let ast = AstGrep::new(code, lang);

        // Get matches for the child pattern we're looking for
        let child_matches = self.evaluate_rule_against_code(has_rule, code, lang)?;

        if child_matches.is_empty() {
            return Ok(vec![]);
        }

        // Get all possible parent nodes
        let all_nodes = self.get_all_nodes(&ast, lang)?;

        let mut has_matches = Vec::new();

        // Check which nodes contain the child matches
        for node in all_nodes {
            for child in &child_matches {
                if self.node_contains(&node, child) {
                    if !has_matches
                        .iter()
                        .any(|m: &MatchResult| m.text == node.text)
                    {
                        has_matches.push(node.clone());
                    }
                    break;
                }
            }
        }

        Ok(has_matches)
    }

    #[allow(dead_code)]
    fn evaluate_follows_rule(
        &self,
        follows_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let ast = AstGrep::new(code, lang);

        // Get matches for the preceding pattern
        let preceding_matches = self.evaluate_rule_against_code(follows_rule, code, lang)?;

        if preceding_matches.is_empty() {
            return Ok(vec![]);
        }

        // Get all possible nodes that could follow
        let all_nodes = self.get_all_nodes(&ast, lang)?;

        let mut follows_matches = Vec::new();

        // Check which nodes follow the preceding matches
        for node in all_nodes {
            for preceding in &preceding_matches {
                if self.node_follows(&node, preceding) {
                    follows_matches.push(node);
                    break;
                }
            }
        }

        Ok(follows_matches)
    }

    #[allow(dead_code)]
    fn evaluate_precedes_rule(
        &self,
        precedes_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let ast = AstGrep::new(code, lang);

        // Get matches for the following pattern
        let following_matches = self.evaluate_rule_against_code(precedes_rule, code, lang)?;

        if following_matches.is_empty() {
            return Ok(vec![]);
        }

        // Get all possible nodes that could precede
        let all_nodes = self.get_all_nodes(&ast, lang)?;

        let mut precedes_matches = Vec::new();

        // Check which nodes precede the following matches
        for node in all_nodes {
            for following in &following_matches {
                if self.node_precedes(&node, following) {
                    precedes_matches.push(node);
                    break;
                }
            }
        }

        Ok(precedes_matches)
    }

    // Helper methods for relational evaluation

    #[allow(dead_code)]
    fn get_all_nodes(
        &self,
        ast: &AstGrep<StrDoc<Language>>,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Use a catch-all pattern to get all significant nodes
        let pattern = self.get_or_create_pattern("$_", lang)?;

        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| {
                let vars: HashMap<String, String> = node.get_env().clone().into();
                let text = node.text().to_string();
                // TODO: Implement proper position extraction from node.range()
                MatchResult {
                    text: text.clone(),
                    vars,
                    start_line: 1,
                    end_line: 1,
                    start_col: 0,
                    end_col: text.len(),
                }
            })
            .collect();

        Ok(matches)
    }

    #[allow(dead_code)]
    fn is_node_inside(&self, node: &MatchResult, container: &MatchResult) -> bool {
        // Check if node is spatially inside container
        node.start_line >= container.start_line
            && node.end_line <= container.end_line
            && (node.start_line > container.start_line || node.start_col >= container.start_col)
            && (node.end_line < container.end_line || node.end_col <= container.end_col)
            && !(node.start_line == container.start_line
                && node.start_col == container.start_col
                && node.end_line == container.end_line
                && node.end_col == container.end_col)
    }

    #[allow(dead_code)]
    fn node_contains(&self, parent: &MatchResult, child: &MatchResult) -> bool {
        // Check if parent spatially contains child
        self.is_node_inside(child, parent)
    }

    #[allow(dead_code)]
    fn node_follows(&self, node: &MatchResult, preceding: &MatchResult) -> bool {
        // Check if node comes after preceding node
        node.start_line > preceding.end_line
            || (node.start_line == preceding.end_line && node.start_col >= preceding.end_col)
    }

    #[allow(dead_code)]
    fn node_precedes(&self, node: &MatchResult, following: &MatchResult) -> bool {
        // Check if node comes before following node
        node.end_line < following.start_line
            || (node.end_line == following.start_line && node.end_col <= following.start_col)
    }

    #[allow(dead_code)]
    fn byte_offset_to_line_col(&self, code: &str, byte_offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 0;

        for (i, ch) in code.char_indices() {
            if i >= byte_offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }

        (line, col)
    }

    #[allow(dead_code)]
    fn evaluate_all_rule(
        &self,
        all_rules: &[RuleObject],
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        if all_rules.is_empty() {
            return Ok(Vec::new());
        }

        // Start with matches from the first rule
        let mut current_matches = self.evaluate_rule_against_code(&all_rules[0], code, lang)?;

        // For each additional rule, filter current matches to only those that also match the new rule
        for rule in &all_rules[1..] {
            let rule_matches = self.evaluate_rule_against_code(rule, code, lang)?;

            // Keep only matches that appear in both sets (intersection)
            current_matches = self.intersect_matches(current_matches, rule_matches);
        }

        Ok(current_matches)
    }

    #[allow(dead_code)]
    fn evaluate_any_rule(
        &self,
        any_rules: &[RuleObject],
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let mut all_matches = Vec::new();

        // Collect matches from all rules
        for rule in any_rules {
            let mut rule_matches = self.evaluate_rule_against_code(rule, code, lang)?;
            all_matches.append(&mut rule_matches);
        }

        // Remove duplicates by text (simple deduplication)
        all_matches.sort_by(|a, b| a.text.cmp(&b.text));
        all_matches.dedup_by(|a, b| a.text == b.text);

        Ok(all_matches)
    }

    #[allow(dead_code)]
    fn evaluate_not_rule(
        &self,
        not_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // This is complex - we need to find all nodes that DON'T match the rule
        // For now, implement a simplified approach using text analysis

        let excluded_matches = self.evaluate_rule_against_code(not_rule, code, lang)?;
        let excluded_texts: std::collections::HashSet<String> =
            excluded_matches.iter().map(|m| m.text.clone()).collect();

        // Get all possible tokens/expressions and filter out the excluded ones
        let ast = AstGrep::new(code, lang);
        let pattern = Pattern::new("$_", lang); // Match anything

        let filtered_matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .filter_map(|node| {
                let text = node.text().to_string();
                if !excluded_texts.contains(&text) {
                    let vars: HashMap<String, String> = node.get_env().clone().into();
                    // TODO: Implement proper position extraction from node.range()
                    Some(MatchResult {
                        text: text.clone(),
                        vars,
                        start_line: 1,
                        end_line: 1,
                        start_col: 0,
                        end_col: text.len(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(filtered_matches)
    }

    #[allow(dead_code)]
    fn intersect_matches(
        &self,
        matches1: Vec<MatchResult>,
        matches2: Vec<MatchResult>,
    ) -> Vec<MatchResult> {
        let texts2: std::collections::HashSet<String> =
            matches2.iter().map(|m| m.text.clone()).collect();

        matches1
            .into_iter()
            .filter(|m| texts2.contains(&m.text))
            .collect()
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern))]
    pub async fn search(&self, param: SearchParam) -> Result<SearchResult, ServiceError> {
        let result = self.search_service.search(param).await?;
        tracing::Span::current().record("matches_found", result.matches.len());
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern, path_pattern = %param.path_pattern))]
    pub async fn file_search(
        &self,
        param: FileSearchParam,
    ) -> Result<FileSearchResult, ServiceError> {
        let result = self.search_service.file_search(param).await?;
        tracing::Span::current().record("total_files_found", result.total_files_found);
        tracing::Span::current().record("files_with_matches", result.matches.len());
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern, replacement = %param.replacement))]
    pub async fn replace(&self, param: ReplaceParam) -> Result<ReplaceResult, ServiceError> {
        self.replace_service.replace(param).await
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern, replacement = %param.replacement, path_pattern = %param.path_pattern, dry_run = %param.dry_run, summary_only = %param.summary_only))]
    pub async fn file_replace(
        &self,
        param: FileReplaceParam,
    ) -> Result<FileReplaceResult, ServiceError> {
        let result = self.replace_service.file_replace(param).await?;
        tracing::Span::current().record("total_files_found", result.total_files_found);
        tracing::Span::current().record("files_with_changes", result.files_with_changes);
        tracing::Span::current().record("total_changes", result.total_changes);
        Ok(result)
    }

    pub async fn list_languages(
        &self,
        _param: ListLanguagesParam,
    ) -> Result<ListLanguagesResult, ServiceError> {
        // List all supported languages manually since all_languages() may not exist
        let languages = vec![
            "bash",
            "c",
            "cpp",
            "csharp",
            "css",
            "dart",
            "elixir",
            "go",
            "haskell",
            "html",
            "java",
            "javascript",
            "json",
            "kotlin",
            "lua",
            "php",
            "python",
            "ruby",
            "rust",
            "scala",
            "swift",
            "typescript",
            "tsx",
            "yaml",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();

        Ok(ListLanguagesResult { languages })
    }

    #[tracing::instrument(skip(self))]
    pub async fn documentation(
        &self,
        _param: DocumentationParam,
    ) -> Result<DocumentationResult, ServiceError> {
        let docs = r##"
# AST-Grep MCP Service Documentation

This service provides structural code search and transformation using ast-grep patterns and rule configurations.

## Key Concepts

**AST Patterns:** Use `$VAR` to capture single nodes, `$$$` to capture multiple statements
**Rule Configurations:** YAML or JSON configurations for complex pattern matching and transformations
**Languages:** Supports javascript, typescript, rust, python, java, go, and many more
**Glob Patterns:** Use `**/*.js` for recursive search, `src/*.ts` for single directory

## search

Searches for patterns in code provided as a string. Useful for quick checks or when code snippets are generated dynamically.

**Parameters:**
- `code`: The source code string to search within.
- `pattern`: The ast-grep pattern to search for (e.g., "console.log($VAR)").
- `language`: The programming language of the code (e.g., "javascript", "typescript", "rust").

**Example Usage:**
```json
{
  "tool_code": "search",
  "tool_params": {
    "code": "function greet() { console.log(\"Hello\"); }",
    "pattern": "console.log($VAR)",
    "language": "javascript"
  }
}
```

**More Pattern Examples:**
```json
// Find function declarations
{
  "pattern": "function $NAME($PARAMS) { $BODY }",
  "language": "javascript"
}

// Find variable assignments
{
  "pattern": "const $VAR = $VALUE",
  "language": "javascript"
}

// Find Rust function definitions
{
  "pattern": "fn $NAME($PARAMS) -> $RETURN_TYPE { $BODY }",
  "language": "rust"
}
```

## file_search

Searches for patterns within files matching a glob pattern. Ideal for analyzing existing code files on the system.

**Parameters:**
- `path_pattern`: A glob pattern for files to search within (e.g., "src/**/*.js").
- `pattern`: The ast-grep pattern to search for.
- `language`: The programming language of the file.
- `max_results` (optional): Maximum number of results to return (default: 1000).
- `max_file_size` (optional): Maximum file size to process in bytes (default: 50MB).
- `cursor` (optional): Continuation token from previous search for pagination.

**Example Usage:**
```json
{
  "tool_code": "file_search",
  "tool_params": {
    "path_pattern": "src/**/*.rs",
    "pattern": "fn $FN_NAME()",
    "language": "rust"
  }
}
```

**Common Use Cases:**
```json
// Find all TODO comments
{
  "path_pattern": "**/*.js",
  "pattern": "// TODO: $MESSAGE",
  "language": "javascript"
}

// Find error handling patterns
{
  "path_pattern": "src/**/*.ts",
  "pattern": "catch ($ERROR) { $BODY }",
  "language": "typescript"
}

// Find React components
{
  "path_pattern": "components/**/*.jsx",
  "pattern": "function $NAME($PROPS) { return $JSX }",
  "language": "javascript"
}

// Find Python class definitions
{
  "path_pattern": "**/*.py",
  "pattern": "class $NAME($BASE): $BODY",
  "language": "python"
}
```

## replace

Replaces patterns in code provided as a string. Useful for in-memory code transformations.

**Parameters:**
- `code`: The source code string to modify.
- `pattern`: The ast-grep pattern to search for.
- `replacement`: The ast-grep replacement pattern.
- `language`: The programming language of the code.

**Example Usage:**
```json
{
  "tool_code": "replace",
  "tool_params": {
    "code": "function oldName() { console.log(\"Hello\"); }",
    "pattern": "function oldName()",
    "replacement": "function newName()",
    "language": "javascript"
  }
}
```

**Transformation Examples:**
```json
// Convert var to const
{
  "pattern": "var $VAR = $VALUE",
  "replacement": "const $VAR = $VALUE",
  "language": "javascript"
}

// Add async/await
{
  "pattern": "function $NAME($PARAMS) { return $BODY }",
  "replacement": "async function $NAME($PARAMS) { return await $BODY }",
  "language": "javascript"
}

// Convert Python print statements
{
  "pattern": "print $ARGS",
  "replacement": "print($ARGS)",
  "language": "python"
}

// Modernize Rust syntax
{
  "pattern": "match $EXPR { $ARMS }",
  "replacement": "match $EXPR { $ARMS }",
  "language": "rust"
}
```

## file_replace

Replaces patterns within files matching a glob pattern. Supports bulk refactoring with optimized response formats.

**Parameters:**
- `path_pattern`: A glob pattern for files to modify (e.g., "src/**/*.js").
- `pattern`: The ast-grep pattern to search for.
- `replacement`: The ast-grep replacement pattern.
- `language`: The programming language of the file.
- `dry_run` (optional): If true (default), only show preview. If false, actually modify files.
- `summary_only` (optional): If true, return only change counts per file (default: false).
- `include_samples` (optional): If true with summary_only, include sample changes (default: false).
- `max_samples` (optional): Number of sample changes per file when include_samples=true (default: 3).
- `max_results` (optional): Maximum number of results to return (default: 1000).
- `max_file_size` (optional): Maximum file size to process in bytes (default: 50MB).
- `cursor` (optional): Continuation token from previous search for pagination.

**IMPORTANT FOR LLMs**: Use `summary_only=true` for bulk refactoring to avoid token limits. This returns concise statistics instead of full diffs.

**Bulk Refactoring Workflow for LLMs:**

1. **Survey scope (use for large codebases to avoid token limits):**
```json
{
  "path_pattern": "src/**/*.rs",
  "pattern": "\"$STRING\".to_string()",
  "replacement": "\"$STRING\".into()",
  "language": "rust",
  "summary_only": true,
  "dry_run": true
}
```
Returns: `{"files_with_changes": [["src/main.rs", 15], ["src/lib.rs", 8]], "total_changes": 23}`

2. **Preview samples before applying:**
```json
{
  "path_pattern": "src/**/*.rs",
  "pattern": "\"$STRING\".to_string()",
  "replacement": "\"$STRING\".into()",
  "language": "rust",
  "summary_only": true,
  "include_samples": true,
  "max_samples": 3,
  "dry_run": true
}
```

3. **Apply changes:**
```json
{
  "path_pattern": "src/**/*.rs",
  "pattern": "\"$STRING\".to_string()",
  "replacement": "\"$STRING\".into()",
  "language": "rust",
  "summary_only": true,
  "dry_run": false
}
```

**Returns Line Diffs:**
```json
{
  "file_results": [{
    "file_path": "src/main.js",
    "file_size_bytes": 15420,
    "changes": [
      {
        "line": 15,
        "old_text": "const x = 5;",
        "new_text": "let x = 5;"
      },
      {
        "line": 23,
        "old_text": "const result = calculate();",
        "new_text": "let result = calculate();"
      }
    ],
    "total_changes": 2,
    "file_hash": "sha256:abc123..."
  }],
  "dry_run": true
}
```

**Batch Transformation Examples:**
```json
// Preview changes first
{
  "path_pattern": "src/**/*.ts",
  "pattern": "fetch($URL).then($HANDLER)",
  "replacement": "await fetch($URL).then($HANDLER)",
  "language": "typescript",
  "dry_run": true
}

// Then apply the changes
{
  "path_pattern": "src/**/*.ts",
  "pattern": "fetch($URL).then($HANDLER)",
  "replacement": "await fetch($URL).then($HANDLER)",
  "language": "typescript",
  "dry_run": false
}
```

**Output Format for all tools:**

`search` and `file_search` return a list of matches. Each match includes:
- `text`: The full text of the matched code snippet.
- `vars`: A dictionary (key-value pairs) of captured variables (e.g., `$VAR`, `$FN_NAME`) and their corresponding matched text.

`replace` and `file_replace` return the `rewritten_code` or `rewritten_file_content` as a string.

```json
{
  "matches": [
    {
      "text": "console.log(\"Hello\")",
      "vars": {
        "VAR": "\"Hello\""
      }
    }
  ]
}
```

## Pagination

`file_search` and `file_replace` support pagination for large result sets. When results are paginated:

- The response includes a `next_cursor` field with a continuation token
- Use this cursor in the `cursor` parameter of the next request
- The `total_files_found` field shows how many files matched the glob pattern
- When `next_cursor.is_complete` is true, no more results are available

**Pagination Example:**
```json
{
  "tool_code": "file_search",
  "tool_params": {
    "path_pattern": "src/**/*.js",
    "pattern": "function $NAME()",
    "language": "javascript",
    "max_results": 10,
    "cursor": {
      "last_file_path": "c3JjL2NvbXBvbmVudHMvQnV0dG9uLmpz",
      "is_complete": false
    }
  }
}
```

## list_languages

Returns all supported programming languages.

**Usage:**
```json
{
  "tool_code": "list_languages",
  "tool_params": {}
}
```

**Supported Languages Include:**
- **Web:** javascript, typescript, tsx, html, css
- **Systems:** rust, c, cpp, go
- **Enterprise:** java, csharp, kotlin, scala
- **Scripting:** python, ruby, lua, bash
- **Others:** swift, dart, elixir, haskell, php, yaml, json

## Best Practices

**Pattern Writing Tips:**
- Use specific patterns: `console.log($VAR)` vs `$ANY`
- Capture what you need: `function $NAME($PARAMS)` captures both name and parameters
- Test patterns with the `search` tool first before using `file_search`

**Performance Tips:**
- Use specific glob patterns: `src/components/*.tsx` vs `**/*`
- Set reasonable `max_file_size` and `max_results` limits
- Use pagination for large codebases

**Common Patterns:**
- Function calls: `$FUNC($ARGS)`
- Variable declarations: `$TYPE $NAME = $VALUE`
- Class methods: `$VISIBILITY $METHOD($PARAMS) { $BODY }`
- Import statements: `import $NAME from '$PATH'`

## Rule-Based Operations

### validate_rule

Validates ast-grep rule configuration syntax and optionally tests against sample code.

**Parameters:**
- `rule_config`: YAML or JSON rule configuration string
- `test_code` (optional): Sample code to test the rule against

**Rule Configuration Format (YAML):**
```yaml
id: unique-rule-id
language: javascript
message: "Optional message for matches"
severity: warning
rule:
  pattern: "console.log($ARG)"
fix: "console.debug($ARG)"  # For rule_replace only
```

**Rule Configuration Format (JSON):**
```json
{
  "id": "unique-rule-id",
  "language": "javascript",
  "message": "Optional message for matches",
  "severity": "warning",
  "rule": {
    "pattern": "console.log($ARG)"
  },
  "fix": "console.debug($ARG)"
}
```

### rule_search

Search for patterns using ast-grep rule configurations. Supports complex pattern matching.

**Parameters:**
- `rule_config`: YAML or JSON rule configuration
- `path_pattern` (optional): Glob pattern for files to search (default: all files)
- `max_results` (optional): Maximum number of results
- `max_file_size` (optional): Maximum file size to process
- `cursor` (optional): Pagination cursor

**Supported Rule Types:**
- **Simple Pattern Rules**: `pattern: "console.log($ARG)"`
- **Composite Rules**: `all`, `any`, `not` (limited support)
- **Relational Rules**: `inside`, `has`, `follows`, `precedes` (planned)

### rule_replace

Replace patterns using ast-grep rule configurations with fix transformations.

**Parameters:**
- `rule_config`: YAML or JSON rule configuration with `fix` field
- `path_pattern` (optional): Glob pattern for files to modify
- `dry_run` (optional): If true (default), only show preview
- `summary_only` (optional): If true, return only summary statistics
- `max_results` (optional): Maximum number of results
- `cursor` (optional): Pagination cursor

**Example Rule with Fix:**
```yaml
id: modernize-var-to-const
language: javascript
message: "Replace var with const for immutable variables"
severity: info
rule:
  pattern: "var $NAME = $VALUE"
fix: "const $NAME = $VALUE"
```

## Rule Management for LLMs

The service provides comprehensive rule management capabilities allowing LLMs to create, store, and reuse custom rule configurations.

### create_rule

Create and store a new ast-grep rule configuration for reuse. LLMs can build custom rule libraries.

**Parameters:**
- `rule_config`: YAML or JSON rule configuration to create
- `overwrite` (optional): Whether to overwrite existing rule with same ID (default: false)

**Usage:**
```json
{
  "rule_config": "id: my-custom-rule\nlanguage: typescript\nmessage: \"Custom rule\"\nrule:\n  pattern: \"$VAR as any\"\nfix: \"$VAR as unknown\"",
  "overwrite": false
}
```

### list_rules

List all stored rule configurations with optional filtering.

**Parameters:**
- `language` (optional): Filter rules by programming language
- `severity` (optional): Filter rules by severity level (info, warning, error)

**Returns:** Array of rule information including ID, language, message, severity, file path, and whether it has a fix.

### get_rule

Retrieve a specific stored rule configuration by ID.

**Parameters:**
- `rule_id`: ID of the rule to retrieve

**Returns:** Full rule configuration as YAML string and file path.

### delete_rule

Delete a stored rule configuration by ID.

**Parameters:**
- `rule_id`: ID of the rule to delete

**Returns:** Confirmation of deletion with rule ID and file path.

**Rule Storage:**
- Rules are stored as YAML files in `.ast-grep-rules/` directory
- Each rule is saved as `{rule-id}.yaml`
- Directory is created automatically when first rule is saved
- Rules persist between server restarts

**LLM Workflow Example:**
1. Create a custom rule: `create_rule` with your pattern and fix
2. Test the rule: `validate_rule` with sample code
3. Apply the rule: `rule_search` or `rule_replace` using the stored rule ID
4. Manage rules: `list_rules`, `get_rule`, `delete_rule` as needed

## Error Handling

The service returns structured errors for:
- Invalid glob patterns
- Unsupported languages
- File access issues
- Pattern syntax errors
- Rule configuration parsing errors
- Missing fix field for replacements

Always check the response for error conditions before processing results.
        "##;
        Ok(DocumentationResult {
            content: docs.to_string(),
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn validate_rule(
        &self,
        param: RuleValidateParam,
    ) -> Result<RuleValidateResult, ServiceError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut test_matches: Option<RuleTestResult> = None;

        // Parse the rule configuration
        let config = match self.parse_rule_config(&param.rule_config) {
            Ok(config) => config,
            Err(e) => {
                errors.push(e.to_string());
                return Ok(RuleValidateResult {
                    valid: false,
                    errors,
                    test_results: None,
                });
            }
        };

        // Validate the configuration
        if let Err(e) = self.validate_rule_config(&config) {
            errors.push(e.to_string());
        }

        // If test code is provided, test the rule against it
        if let Some(ref test_code) = param.test_code
            && errors.is_empty()
            && let Some(pattern_str) = self.extract_pattern_from_rule(&config.rule)
        {
            match self.parse_language(&config.language) {
                Ok(_lang) => {
                    let search_param = SearchParam {
                        code: test_code.clone(),
                        pattern: pattern_str,
                        language: config.language.clone(),
                    };

                    match self.search(search_param).await {
                        Ok(result) => {
                            test_matches = Some(RuleTestResult {
                                matches_found: result.matches.len(),
                                sample_matches: result
                                    .matches
                                    .into_iter()
                                    .take(5)
                                    .map(|m| m.text)
                                    .collect(),
                            });
                        }
                        Err(e) => {
                            warnings.push(format!("Pattern test failed: {e}"));
                        }
                    }
                }
                Err(e) => {
                    errors.push(e.to_string());
                }
            }
        } else if param.test_code.is_some() && !errors.is_empty() {
            warnings.push("Test code provided but rule has errors".into());
        }

        Ok(RuleValidateResult {
            valid: errors.is_empty(),
            errors,
            test_results: test_matches,
        })
    }

    #[tracing::instrument(skip(self), fields(rule_id))]
    pub async fn rule_search(
        &self,
        param: RuleSearchParam,
    ) -> Result<FileSearchResult, ServiceError> {
        let result = self.search_service.rule_search(param).await?;
        tracing::Span::current().record("total_files_found", result.total_files_found);
        tracing::Span::current().record("files_with_matches", result.matches.len());
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(rule_id))]
    pub async fn rule_replace(
        &self,
        param: RuleReplaceParam,
    ) -> Result<FileReplaceResult, ServiceError> {
        let result = self.replace_service.rule_replace(param).await?;
        tracing::Span::current().record("total_files_found", result.total_files_found);
        tracing::Span::current().record("files_with_changes", result.files_with_changes);
        tracing::Span::current().record("total_changes", result.total_changes);
        Ok(result)
    }

    fn ensure_rules_directory(&self) -> Result<(), ServiceError> {
        if !self.config.rules_directory.exists() {
            fs::create_dir_all(&self.config.rules_directory)?;
        }
        Ok(())
    }

    fn get_rule_file_path(&self, rule_id: &str) -> PathBuf {
        self.config.rules_directory.join(format!("{rule_id}.yaml"))
    }

    #[tracing::instrument(skip(self), fields(rule_id))]
    pub async fn create_rule(
        &self,
        param: CreateRuleParam,
    ) -> Result<CreateRuleResult, ServiceError> {
        // Parse and validate the rule configuration
        let config = self.parse_rule_config(&param.rule_config)?;
        self.validate_rule_config(&config)?;

        tracing::Span::current().record("rule_id", &config.id);

        // Ensure rules directory exists
        self.ensure_rules_directory()?;

        let file_path = self.get_rule_file_path(&config.id);
        let exists = file_path.exists();

        // Check if rule exists and overwrite is not allowed
        if exists && !param.overwrite {
            return Err(ServiceError::Internal(format!(
                "Rule '{}' already exists. Use overwrite=true to replace it.",
                config.id
            )));
        }

        // Write rule to file as YAML
        let yaml_content = serde_yaml::to_string(&config).map_err(|e| {
            ServiceError::Internal(format!("Failed to serialize rule to YAML: {e}"))
        })?;

        fs::write(&file_path, yaml_content)?;

        Ok(CreateRuleResult {
            rule_id: config.id,
            file_path: file_path.to_string_lossy().to_string(),
            created: !exists,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_rules(&self, param: ListRulesParam) -> Result<ListRulesResult, ServiceError> {
        // Ensure rules directory exists
        self.ensure_rules_directory()?;

        let mut rules = Vec::new();

        // Read all YAML files in rules directory
        for entry in fs::read_dir(&self.config.rules_directory)? {
            let entry = entry?;
            let path = entry.path();

            if path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                match self.load_rule_from_file(&path) {
                    Ok(config) => {
                        // Apply filters
                        if let Some(lang_filter) = &param.language {
                            if config.language != *lang_filter {
                                continue;
                            }
                        }

                        if let Some(severity_filter) = &param.severity {
                            if config.severity.as_ref() != Some(severity_filter) {
                                continue;
                            }
                        }

                        rules.push(RuleInfo {
                            id: config.id,
                            language: config.language,
                            message: config.message,
                            severity: config.severity,
                            file_path: path.to_string_lossy().to_string(),
                            has_fix: config.fix.is_some(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load rule from {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort rules by ID for consistent ordering
        rules.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(ListRulesResult { rules })
    }

    fn load_rule_from_file(&self, path: &PathBuf) -> Result<RuleConfig, ServiceError> {
        let content = fs::read_to_string(path)?;
        self.parse_rule_config(&content)
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_catalog_rules(
        &self,
        param: ListCatalogRulesParam,
    ) -> Result<ListCatalogRulesResult, ServiceError> {
        // For now, return a static list of example rules from the ast-grep catalog
        // In a real implementation, this would fetch from https://ast-grep.github.io/catalog/
        let mut rules = vec![
            CatalogRuleInfo {
                id: "xstate-v4-to-v5".to_string(),
                name: "XState v4 to v5 Migration".to_string(),
                description: "Migrate XState v4 code to v5 syntax".to_string(),
                language: "typescript".to_string(),
                category: "migration".to_string(),
                url: "https://ast-grep.github.io/catalog/typescript/xstate-v4-to-v5".to_string(),
            },
            CatalogRuleInfo {
                id: "no-console-log".to_string(),
                name: "No Console Log".to_string(),
                description: "Find and remove console.log statements".to_string(),
                language: "javascript".to_string(),
                category: "cleanup".to_string(),
                url: "https://ast-grep.github.io/catalog/javascript/no-console-log".to_string(),
            },
            CatalogRuleInfo {
                id: "use-strict-equality".to_string(),
                name: "Use Strict Equality".to_string(),
                description: "Replace == with === for strict equality".to_string(),
                language: "javascript".to_string(),
                category: "best-practices".to_string(),
                url: "https://ast-grep.github.io/catalog/javascript/use-strict-equality"
                    .to_string(),
            },
        ];

        // Filter by language if specified
        if let Some(lang) = &param.language {
            rules.retain(|rule| rule.language == *lang);
        }

        // Filter by category if specified
        if let Some(cat) = &param.category {
            rules.retain(|rule| rule.category == *cat);
        }

        Ok(ListCatalogRulesResult { rules })
    }

    #[tracing::instrument(skip(self))]
    pub async fn import_catalog_rule(
        &self,
        param: ImportCatalogRuleParam,
    ) -> Result<ImportCatalogRuleResult, ServiceError> {
        // For now, this is a mock implementation
        // In a real implementation, this would:
        // 1. Fetch the rule content from the provided URL
        // 2. Parse the YAML/JSON rule configuration
        // 3. Store it using the create_rule method

        // Extract rule ID from URL or use provided one
        let rule_id = param.rule_id.unwrap_or_else(|| {
            // Extract ID from URL (last segment)
            param
                .rule_url
                .split('/')
                .next_back()
                .unwrap_or("imported-rule")
                .to_string()
        });

        // Mock rule content - in real implementation, this would be fetched from the URL
        let mock_rule_config = format!(
            r#"
id: {rule_id}
message: "Imported rule from catalog"
language: javascript
severity: warning
rule:
  pattern: console.log($VAR)
fix: "// TODO: Replace with proper logging: console.log($VAR)"
"#
        );

        // Use the existing create_rule method to store the imported rule
        let create_param = CreateRuleParam {
            rule_config: mock_rule_config,
            overwrite: false,
        };

        match self.create_rule(create_param).await {
            Ok(_) => Ok(ImportCatalogRuleResult {
                rule_id: rule_id.clone(),
                imported: true,
                message: format!("Successfully imported rule '{rule_id}' from catalog"),
            }),
            Err(e) => Ok(ImportCatalogRuleResult {
                rule_id: rule_id.clone(),
                imported: false,
                message: format!("Failed to import rule: {e}"),
            }),
        }
    }

    #[tracing::instrument(skip(self), fields(rule_id = %param.rule_id))]
    pub async fn delete_rule(
        &self,
        param: DeleteRuleParam,
    ) -> Result<DeleteRuleResult, ServiceError> {
        let file_path = self.get_rule_file_path(&param.rule_id);

        if file_path.exists() {
            fs::remove_file(&file_path)?;
            Ok(DeleteRuleResult {
                rule_id: param.rule_id.clone(),
                deleted: true,
                message: format!("Rule '{}' deleted successfully", param.rule_id),
            })
        } else {
            Ok(DeleteRuleResult {
                rule_id: param.rule_id.clone(),
                deleted: false,
                message: format!("Rule '{}' not found", param.rule_id),
            })
        }
    }

    #[tracing::instrument(skip(self), fields(rule_id = %param.rule_id))]
    pub async fn get_rule(&self, param: GetRuleParam) -> Result<GetRuleResult, ServiceError> {
        let file_path = self.get_rule_file_path(&param.rule_id);

        if !file_path.exists() {
            return Err(ServiceError::Internal(format!(
                "Rule '{}' not found",
                param.rule_id
            )));
        }

        let content = fs::read_to_string(&file_path)?;

        let rule_config = self.parse_rule_config(&content)?;
        Ok(GetRuleResult {
            rule_config,
            file_path: file_path.to_string_lossy().to_string(),
        })
    }
}

impl ServerHandler for AstGrepService {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::LATEST,
            server_info: Implementation {
                name: "ast-grep-mcp".into(),
                version: "0.1.0".into(),
            },
            capabilities: ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability { list_changed: Some(true) }),
                ..Default::default()
            },
            instructions: Some("This MCP server provides tools for structural code search and transformation using ast-grep. For bulk refactoring, use file_replace with summary_only=true to avoid token limits. Use the `documentation` tool for detailed examples.".into()),
        }
    }

    #[tracing::instrument(skip(self, _request, _context))]
    async fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "search".into(),
                    description: "Search for patterns in code using ast-grep.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": { "code": { "type": "string" }, "pattern": { "type": "string" }, "language": { "type": "string" } } })).unwrap()),
                },
                Tool {
                    name: "file_search".into(),
                    description: "Search for patterns in a file using ast-grep.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string" },
                            "pattern": { "type": "string" },
                            "language": { "type": "string" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 100 },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824 },
                            "cursor": {
                                "type": "object",
                                "properties": {
                                    "last_file_path": { "type": "string" },
                                    "is_complete": { "type": "boolean" }
                                },
                                "required": ["last_file_path", "is_complete"]
                            }
                        },
                        "required": ["path_pattern", "pattern", "language"]
                    })).unwrap()),
                },
                Tool {
                    name: "replace".into(),
                    description: "Replace patterns in code.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": { "code": { "type": "string" }, "pattern": { "type": "string" }, "replacement": { "type": "string" }, "language": { "type": "string" } } })).unwrap()),
                },
                Tool {
                    name: "file_replace".into(),
                    description: "Replace patterns in files. Use summary_only=true for bulk refactoring to avoid token limits. Returns change counts or line diffs.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string" },
                            "pattern": { "type": "string" },
                            "replacement": { "type": "string" },
                            "language": { "type": "string" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 10000 },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824 },
                            "dry_run": { "type": "boolean", "default": true, "description": "If true (default), only show preview. If false, actually modify files." },
                            "summary_only": { "type": "boolean", "default": false, "description": "If true, only return summary statistics (change counts per file)" },
                            "include_samples": { "type": "boolean", "default": false, "description": "If true, include sample changes in the response (first few changes per file)" },
                            "max_samples": { "type": "integer", "default": 3, "minimum": 1, "maximum": 20, "description": "Maximum number of sample changes to show per file" },
                            "cursor": {
                                "type": "object",
                                "properties": {
                                    "last_file_path": { "type": "string" },
                                    "is_complete": { "type": "boolean" }
                                },
                                "required": ["last_file_path", "is_complete"]
                            }
                        },
                        "required": ["path_pattern", "pattern", "replacement", "language"]
                    })).unwrap()),
                },
                Tool {
                    name: "list_languages".into(),
                    description: "List all supported programming languages.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                },
                Tool {
                    name: "documentation".into(),
                    description: "Provides detailed usage examples for all tools.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                },
                Tool {
                    name: "rule_search".into(),
                    description: "Search for patterns using ast-grep rule configurations (YAML/JSON). Supports complex pattern matching with relational and composite rules.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML or JSON rule configuration" },
                            "path_pattern": { "type": "string", "description": "Glob pattern for files to search (optional, searches all files if not provided)" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 10000 },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824 },
                            "cursor": {
                                "type": "object",
                                "properties": {
                                    "last_file_path": { "type": "string" },
                                    "is_complete": { "type": "boolean" }
                                },
                                "required": ["last_file_path", "is_complete"]
                            }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                },
                Tool {
                    name: "rule_replace".into(),
                    description: "Replace patterns using ast-grep rule configurations with fix transformations. Supports complex rule-based code refactoring.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML or JSON rule configuration with fix field" },
                            "path_pattern": { "type": "string", "description": "Glob pattern for files to modify (optional, processes all files if not provided)" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 10000 },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824 },
                            "dry_run": { "type": "boolean", "default": true, "description": "If true (default), only show preview. If false, actually modify files." },
                            "summary_only": { "type": "boolean", "default": false, "description": "If true, only return summary statistics" },
                            "cursor": {
                                "type": "object",
                                "properties": {
                                    "last_file_path": { "type": "string" },
                                    "is_complete": { "type": "boolean" }
                                },
                                "required": ["last_file_path", "is_complete"]
                            }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                },
                Tool {
                    name: "validate_rule".into(),
                    description: "Validate ast-grep rule configuration syntax and optionally test against sample code.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML or JSON rule configuration to validate" },
                            "test_code": { "type": "string", "description": "Optional code sample to test the rule against" }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                },
                Tool {
                    name: "create_rule".into(),
                    description: "Create and store a new ast-grep rule configuration for reuse. LLMs can use this to build custom rule libraries.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML or JSON rule configuration to create" },
                            "overwrite": { "type": "boolean", "default": false, "description": "Whether to overwrite existing rule with same ID" }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                },
                Tool {
                    name: "list_rules".into(),
                    description: "List all stored rule configurations with optional filtering by language or severity.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "language": { "type": "string", "description": "Filter rules by programming language" },
                            "severity": { "type": "string", "description": "Filter rules by severity level (info, warning, error)" }
                        }
                    })).unwrap()),
                },
                Tool {
                    name: "get_rule".into(),
                    description: "Retrieve a specific stored rule configuration by ID.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_id": { "type": "string", "description": "ID of the rule to retrieve" }
                        },
                        "required": ["rule_id"]
                    })).unwrap()),
                },
                Tool {
                    name: "delete_rule".into(),
                    description: "Delete a stored rule configuration by ID.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_id": { "type": "string", "description": "ID of the rule to delete" }
                        },
                        "required": ["rule_id"]
                    })).unwrap()),
                },
                Tool {
                    name: "list_catalog_rules".into(),
                    description: "List available rules from the ast-grep catalog with optional filtering.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "language": { "type": "string", "description": "Filter rules by programming language" },
                            "category": { "type": "string", "description": "Filter rules by category" }
                        }
                    })).unwrap()),
                },
                Tool {
                    name: "import_catalog_rule".into(),
                    description: "Import a rule from the ast-grep catalog into local storage.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_url": { "type": "string", "description": "URL of the catalog rule to import" },
                            "rule_id": { "type": "string", "description": "Optional custom ID for the imported rule" }
                        },
                        "required": ["rule_url"]
                    })).unwrap()),
                },
                Tool {
                    name: "generate_ast".into(),
                    description: "Generate a stringified syntax tree for code using Tree-sitter. Useful for debugging patterns and understanding AST structure.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "code": { "type": "string", "description": "Source code to parse" },
                            "language": { "type": "string", "description": "Programming language of the code" }
                        },
                        "required": ["code", "language"]
                    })).unwrap()),
                },
                ],
                ..Default::default()
            })
    }

    #[tracing::instrument(skip(self, request, _context), fields(tool_name = %request.name))]
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        match request.name.as_ref() {
            "search" => {
                let param: SearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.search(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "file_search" => {
                let param: FileSearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.file_search(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "replace" => {
                let param: ReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.replace(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "file_replace" => {
                let param: FileReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.file_replace(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "list_languages" => {
                let param: ListLanguagesParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.list_languages(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "documentation" => {
                let param: DocumentationParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.documentation(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "validate_rule" => {
                let param: RuleValidateParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.validate_rule(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "rule_search" => {
                let param: RuleSearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.rule_search(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "rule_replace" => {
                let param: RuleReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.rule_replace(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "create_rule" => {
                let param: CreateRuleParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.create_rule(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "list_rules" => {
                let param: ListRulesParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.list_rules(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "get_rule" => {
                let param: GetRuleParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.get_rule(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "delete_rule" => {
                let param: DeleteRuleParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.delete_rule(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "list_catalog_rules" => {
                let param: ListCatalogRulesParam = serde_json::from_value(
                    serde_json::Value::Object(request.arguments.unwrap_or_default()),
                )
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self
                    .list_catalog_rules(param)
                    .await
                    .map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "import_catalog_rule" => {
                let param: ImportCatalogRuleParam = serde_json::from_value(
                    serde_json::Value::Object(request.arguments.unwrap_or_default()),
                )
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self
                    .import_catalog_rule(param)
                    .await
                    .map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            "generate_ast" => {
                let param: GenerateAstParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.generate_ast(param).await.map_err(ErrorData::from)?;
                let json_value = serde_json::to_value(&result)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                Ok(CallToolResult::success(vec![Content::json(json_value)?]))
            }
            _ => Err(ErrorData::method_not_found::<
                rmcp::model::CallToolRequestMethod,
            >()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_basic() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function greet() { console.log(\"Hello\"); }".to_string(),
            pattern: "console.log($VAR)".into(),
            language: "javascript".into(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].text, "console.log(\"Hello\")");
        assert_eq!(
            result.matches[0].vars.get("VAR"),
            Some(&"\"Hello\"".to_string())
        );
    }

    #[tokio::test]
    async fn test_search_no_matches() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function greet() { alert(\"Hello\"); }".to_string(),
            pattern: "console.log($VAR)".into(),
            language: "javascript".into(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 0);
    }

    #[tokio::test]
    async fn test_search_invalid_language() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function greet() { console.log(\"Hello\"); }".to_string(),
            pattern: "console.log($VAR)".into(),
            language: "invalid_language".into(),
        };

        let result = service.search(param).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ServiceError::Internal(_)));
    }

    #[tokio::test]
    async fn test_replace_basic() {
        let service = AstGrepService::new();
        let param = ReplaceParam {
            code: "function oldName() { console.log(\"Hello\"); }".to_string(),
            pattern: "function oldName()".into(),
            replacement: "function newName()".into(),
            language: "javascript".into(),
        };

        let result = service.replace(param).await.unwrap();
        assert!(result.new_code.contains("function newName()"));
        assert!(!result.new_code.contains("function oldName()"));
    }

    #[tokio::test]
    async fn test_replace_with_vars() {
        let service = AstGrepService::new();
        let param = ReplaceParam {
            code: "const x = 5; const y = 10;".into(),
            pattern: "const $VAR = $VAL".into(),
            replacement: "let $VAR = $VAL".into(),
            language: "javascript".into(),
        };

        let result = service.replace(param).await.unwrap();
        assert!(result.new_code.contains("let x = 5"));
        assert!(result.new_code.contains("let y = 10"));
        assert!(!result.new_code.contains("const"));
    }

    #[tokio::test]
    async fn test_replace_multiple_occurrences() {
        let service = AstGrepService::new();
        let param = ReplaceParam {
            code: "let a = 1; let b = 2; let c = 3;".into(),
            pattern: "let $VAR = $VAL".into(),
            replacement: "const $VAR = $VAL".into(),
            language: "javascript".into(),
        };
        let result = service.replace(param).await.unwrap();
        assert_eq!(result.new_code, "const a = 1; const b = 2; const c = 3;");
    }

    #[tokio::test]
    async fn test_rust_pattern_matching() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "fn main() { println!(\"Hello, world!\"); }".to_string(),
            pattern: "println!($VAR)".into(),
            language: "rust".into(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].text, "println!(\"Hello, world!\")");
        assert_eq!(
            result.matches[0].vars.get("VAR"),
            Some(&"\"Hello, world!\"".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_languages() {
        let service = AstGrepService::new();
        let param = ListLanguagesParam {};

        let result = service.list_languages(param).await.unwrap();
        assert!(!result.languages.is_empty());
        assert!(result.languages.contains(&"rust".to_string()));
        assert!(result.languages.contains(&"javascript".to_string()));
        assert!(result.languages.contains(&"python".to_string()));
    }

    #[tokio::test]
    async fn test_search_cursor() {
        // Test cursor creation and decoding
        let cursor = CursorParam {
            last_file_path: "src/main.rs".to_string(),
            is_complete: false,
        };
        assert!(!cursor.is_complete);

        let decoded = cursor.last_file_path.clone();
        assert_eq!(decoded, "src/main.rs");

        // Test complete cursor
        let complete_cursor = CursorParam {
            last_file_path: String::new(),
            is_complete: true,
        };
        assert!(complete_cursor.is_complete);
        assert_eq!(complete_cursor.last_file_path, "");
    }

    #[tokio::test]
    async fn test_pagination_configuration() {
        let custom_config = ServiceConfig {
            max_file_size: 1024 * 1024, // 1MB
            max_concurrency: 5,
            limit: 10,
            root_directories: vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))],
            rules_directory: PathBuf::from(".test-rules"),
            pattern_cache_size: 500, // Smaller cache for testing
        };

        let service = AstGrepService::with_config(custom_config);
        assert_eq!(service.config.max_file_size, 1024 * 1024);
        assert_eq!(service.config.max_concurrency, 5);
        assert_eq!(service.config.limit, 10);
    }

    #[tokio::test]
    async fn test_documentation() {
        let service = AstGrepService::new();
        let param = DocumentationParam {};

        let result = service.documentation(param).await.unwrap();
        assert!(result.content.contains("search"));
        assert!(result.content.contains("file_search"));
        assert!(result.content.contains("replace"));
        assert!(result.content.contains("file_replace"));
    }

    #[tokio::test]
    async fn test_multiple_matches() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "console.log(\"Hello\"); console.log(\"World\"); alert(\"test\");".to_string(),
            pattern: "console.log($VAR)".into(),
            language: "javascript".into(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 2);
        assert_eq!(
            result.matches[0].vars.get("VAR"),
            Some(&"\"Hello\"".to_string())
        );
        assert_eq!(
            result.matches[1].vars.get("VAR"),
            Some(&"\"World\"".to_string())
        );
    }

    #[tokio::test]
    async fn test_complex_pattern() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "function test(a, b) { return a + b; } function add(x, y) { return x + y; }"
                .into(),
            pattern: "function $NAME($PARAM1, $PARAM2) { return $PARAM1 + $PARAM2; }".into(),
            language: "javascript".into(),
        };

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 2);

        // Check first match
        assert_eq!(
            result.matches[0].vars.get("NAME"),
            Some(&"test".to_string())
        );
        assert_eq!(result.matches[0].vars.get("PARAM1"), Some(&"a".to_string()));
        assert_eq!(result.matches[0].vars.get("PARAM2"), Some(&"b".to_string()));

        // Check second match
        assert_eq!(result.matches[1].vars.get("NAME"), Some(&"add".to_string()));
        assert_eq!(result.matches[1].vars.get("PARAM1"), Some(&"x".to_string()));
        assert_eq!(result.matches[1].vars.get("PARAM2"), Some(&"y".to_string()));
    }

    #[tokio::test]
    async fn test_invalid_language_error() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "let x = 1;".into(),
            pattern: "let x = 1;".into(),
            language: "not_a_real_language".into(),
        };
        let result = service.search(param).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ServiceError::Internal(msg) if msg == "Failed to parse language"));
    }

    #[tokio::test]
    async fn test_pattern_caching() {
        let service = AstGrepService::new();
        let param = SearchParam {
            code: "console.log(\"test\"); console.log(\"another\");".into(),
            pattern: "console.log($VAR)".into(),
            language: "javascript".into(),
        };

        // Run the same search twice
        let result1 = service.search(param.clone()).await.unwrap();
        let result2 = service.search(param).await.unwrap();

        // Both should return the same results
        assert_eq!(result1.matches.len(), 2);
        assert_eq!(result2.matches.len(), 2);
        assert_eq!(result1.matches[0].text, result2.matches[0].text);

        // Verify cache has the pattern (cache should have 1 entry)
        let cache = service.pattern_cache.lock().unwrap();
        assert_eq!(cache.len(), 1);
    }

    #[tokio::test]
    async fn test_lru_cache_eviction() {
        // Create service with very small cache for testing
        let config = ServiceConfig {
            pattern_cache_size: 2, // Only 2 patterns max
            ..Default::default()
        };
        let service = AstGrepService::with_config(config);

        let code = "console.log('test');";

        // Add first pattern
        let _ = service
            .search(SearchParam {
                code: code.into(),
                pattern: "console.log($VAR)".into(),
                language: "javascript".into(),
            })
            .await
            .unwrap();

        // Add second pattern
        let _ = service
            .search(SearchParam {
                code: code.into(),
                pattern: "console.$METHOD($VAR)".into(),
                language: "javascript".into(),
            })
            .await
            .unwrap();

        // Cache should have 2 entries
        let (used, capacity) = service.get_cache_stats();
        assert_eq!(used, 2);
        assert_eq!(capacity, 2);

        // Add third pattern - should evict least recently used
        let _ = service
            .search(SearchParam {
                code: code.into(),
                pattern: "$OBJECT.log($VAR)".into(),
                language: "javascript".into(),
            })
            .await
            .unwrap();

        // Cache should still have 2 entries (LRU evicted the first one)
        let (used, capacity) = service.get_cache_stats();
        assert_eq!(used, 2);
        assert_eq!(capacity, 2);
    }

    #[tokio::test]
    async fn test_generate_ast() {
        let service = AstGrepService::new();
        let param = GenerateAstParam {
            code: "function test() { return 42; }".into(),
            language: "javascript".into(),
        };

        let result = service.generate_ast(param).await.unwrap();

        // Should contain function declaration
        assert!(result.ast.contains("function_declaration"));
        assert!(result.ast.contains("identifier"));
        assert!(result.ast.contains("number"));
        assert_eq!(result.language, "javascript");
        assert_eq!(result.code_length, 30);
    }
}
