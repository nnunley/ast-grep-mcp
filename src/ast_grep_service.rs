use crate::ast_utils::AstParser;
use crate::config::ServiceConfig;
use crate::errors::ServiceError;
use crate::pattern::PatternMatcher;
use crate::replace::ReplaceService;
use crate::response_formatter::ResponseFormatter;
use crate::rules::*;
use crate::rules::{RuleEvaluator, RuleService, RuleStorage};
use crate::search::SearchService;
use crate::tool_router::ToolRouter;
use crate::types::*;

use ast_grep_core::{AstGrep, Pattern};

use lru::LruCache;
use std::num::NonZeroUsize;
use std::{borrow::Cow, str::FromStr, sync::Arc, sync::Mutex};

use ast_grep_language::SupportLang as Language;

const ALL_LANGUAGES: &[&str] = &[
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
];
// Removed unused base64 import
use rmcp::{
    ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, ErrorData, Implementation, InitializeResult,
        ListToolsResult, PaginatedRequestParam, ProtocolVersion, ServerCapabilities,
    },
    service::{RequestContext, RoleServer},
};
// Removed unused serde imports
use sha2::{Digest, Sha256};

#[derive(Clone)]
#[allow(dead_code)]
pub struct AstGrepService {
    pub(crate) config: ServiceConfig,
    pub(crate) pattern_cache: Arc<Mutex<LruCache<String, Pattern>>>,
    pub(crate) pattern_matcher: PatternMatcher,
    pub(crate) rule_evaluator: RuleEvaluator,
    pub(crate) search_service: SearchService,
    pub(crate) replace_service: ReplaceService,
    pub(crate) rule_service: RuleService,
}

impl Default for AstGrepService {
    fn default() -> Self {
        Self::new()
    }
}

impl AstGrepService {
    fn parse_language(&self, lang_str: &str) -> Result<Language, ServiceError> {
        Language::from_str(lang_str).map_err(|_| {
            let (ast_structure, node_kinds) = self.get_ast_debug_info("", lang_str);
            ServiceError::AstAnalysisError {
                message: format!(
                    "Unsupported language: '{lang_str}'. Please use one of the supported languages."
                ),
                code: "".to_string(), // No specific code to analyze for language error
                language: lang_str.to_string(),
                ast_structure,
                node_kinds,
            }
        })
    }

    /// Extract unique Tree-sitter node kinds from the given code
    /// This is useful for users to discover what node kinds are available for use in Kind rules
    fn extract_node_kinds(&self, code: &str, lang: Language) -> Result<Vec<String>, ServiceError> {
        let ast = AstGrep::new(code, lang);

        // Use a catch-all pattern to find all nodes
        let pattern = Pattern::new("$_", lang);

        let mut unique_kinds = std::collections::HashSet::new();
        for node_match in ast.root().find_all(pattern) {
            unique_kinds.insert(node_match.get_node().kind().to_string());
        }

        let mut kinds: Vec<String> = unique_kinds.into_iter().collect();
        kinds.sort();
        Ok(kinds)
    }

    /// Generate a simple metavariable pattern from code examples
    ///
    /// Helper to get AST debug info for error reporting
    pub fn get_ast_debug_info(&self, code: &str, language_str: &str) -> (String, Vec<String>) {
        let lang_result = Language::from_str(language_str);
        let ast_parser = AstParser::new();

        match lang_result {
            Ok(lang) => {
                let ast_string = ast_parser.generate_ast_debug_string(code, lang);
                let node_kinds = self.extract_node_kinds(code, lang).unwrap_or_default();
                (ast_string, node_kinds)
            }
            Err(_) => {
                // If language parsing fails, return empty AST and all supported languages as kinds
                (
                    "Failed to parse language, cannot generate AST.".to_string(),
                    ALL_LANGUAGES.iter().map(|&s| s.to_string()).collect(),
                )
            }
        }
    }

    pub fn new() -> Self {
        Self::with_config(ServiceConfig::default())
    }

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
        let rule_storage = RuleStorage::with_directories(config.all_rule_directories());
        let rule_service = RuleService::new(config.clone(), rule_evaluator.clone(), rule_storage);

        Self {
            config,
            pattern_cache,
            pattern_matcher,
            rule_evaluator,
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

    #[allow(dead_code)]
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
        let ast_parser = AstParser::new();
        let ast_string = ast_parser.generate_ast_debug_string(&param.code, lang);

        // Extract unique node kinds from the AST
        let node_kinds = self.extract_node_kinds(&param.code, lang)?;

        Ok(GenerateAstResult {
            ast: ast_string,
            language: param.language,
            code_length: param.code.chars().count(),
            node_kinds,
        })
    }

    fn parse_rule_config(&self, rule_config_str: &str) -> Result<RuleConfig, ServiceError> {
        // First try to parse as YAML
        if let Ok(config) = serde_yaml::from_str::<RuleConfig>(rule_config_str) {
            return Ok(config);
        }

        // If YAML fails, try JSON
        serde_json::from_str::<RuleConfig>(rule_config_str).map_err(|e| {
            let (ast_structure, node_kinds) = self.get_ast_debug_info(rule_config_str, "yaml"); // Assuming rule config is YAML
            ServiceError::AstAnalysisError {
                message: format!("Failed to parse rule config as YAML or JSON: {e}"),
                code: rule_config_str.to_string(),
                language: "yaml".to_string(), // Or "json" if it was tried as JSON
                ast_structure,
                node_kinds,
            }
        })
    }

    fn validate_rule_config(&self, config: &RuleConfig) -> Result<(), ServiceError> {
        // Validate language
        self.parse_language(&config.language)?;

        // Validate rule has at least one condition
        if !self.has_rule_condition(&config.rule) {
            let rule_str = serde_json::to_string(&config.rule).unwrap_or_else(|_| "{}".to_string());
            let (ast_structure, node_kinds) = self.get_ast_debug_info(&rule_str, &config.language);
            return Err(ServiceError::AstAnalysisError {
                message: "Rule must have at least one condition (e.g., pattern, kind, regex)."
                    .to_string(),
                code: rule_str,
                language: config.language.clone(),
                ast_structure,
                node_kinds,
            });
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
        let result = self.replace_service.replace(param).await?;
        tracing::Span::current().record("changes_made", result.changes.len());
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern, path_pattern = %param.path_pattern, replacement = %param.replacement, dry_run = %param.dry_run))]
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
        Ok(ListLanguagesResult {
            languages: ALL_LANGUAGES.iter().map(|&s| s.to_string()).collect(),
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
                    let search_param = SearchParam::new(test_code, &pattern_str, &config.language);

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
            warnings.push("Test code provided but rule has errors".to_string());
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

    #[tracing::instrument(skip(self), fields(rule_id))]
    pub async fn create_rule(
        &self,
        param: CreateRuleParam,
    ) -> Result<CreateRuleResult, ServiceError> {
        self.rule_service.storage().create_rule(param).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_rules(&self, param: ListRulesParam) -> Result<ListRulesResult, ServiceError> {
        self.rule_service.storage().list_rules(param).await
    }

    #[tracing::instrument(skip(self), fields(rule_id = %param.rule_id))]
    pub async fn delete_rule(
        &self,
        param: DeleteRuleParam,
    ) -> Result<DeleteRuleResult, ServiceError> {
        self.rule_service.storage().delete_rule(param).await
    }

    #[tracing::instrument(skip(self), fields(rule_id = %param.rule_id))]
    pub async fn get_rule(&self, param: GetRuleParam) -> Result<GetRuleResult, ServiceError> {
        self.rule_service.storage().get_rule(param).await
    }
}

impl ServerHandler for AstGrepService {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::LATEST,
            server_info: Implementation {
                name: "ast-grep-mcp".to_string(),
                version: "0.1.0".to_string(),
            },
            capabilities: ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability { list_changed: Some(true) }),
                ..Default::default()
            },
            instructions: Some("AST-Grep MCP Server: Structural code search and transformation using Tree-sitter AST patterns. Supports 20+ languages including JavaScript, TypeScript, Python, Rust, Java, Go. IMPORTANT: Use $VAR for single node captures, and $ for multiple node (list) captures. When searching/replacing, use 'search' or 'replace' for code snippets (requires 'code' parameter). Use 'file_search' or 'file_replace' for operations across files (requires 'path_pattern' parameter). For bulk changes, ALWAYS use 'file_replace' with 'dry_run: true' first to preview changes. For complex logic, use rule-based tools ('rule_search', 'rule_replace', 'validate_rule') with YAML configurations. Refer to TOOL_USAGE_GUIDE.md for comprehensive examples and advanced usage.".to_string()),
        }
    }

    #[tracing::instrument(skip(self, _request, _context))]
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(crate::tools::ToolService::list_tools())
    }

    #[tracing::instrument(skip(self, request, _context), fields(tool_name = %request.name))]
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        // Special handling for file_search with large results
        if request.name == "file_search" {
            return self.handle_file_search_with_optimization(request).await;
        }

        // Special handling for list_languages which has custom implementation
        match request.name.as_ref() {
            "list_languages" => self.handle_list_languages_tool(request).await,
            _ => ToolRouter::route_tool_call(self, request).await,
        }
    }
}

impl AstGrepService {
    /// Helper method to handle file_search with response optimization
    async fn handle_file_search_with_optimization(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: FileSearchParam = serde_json::from_value(serde_json::Value::Object(
            request.arguments.clone().unwrap_or_default(),
        ))
        .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;

        let result = self.file_search(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_file_search_result(&result);

        // Use lightweight response for large results to avoid token limits
        let total_matches: usize = result.matches.iter().map(|f| f.matches.len()).sum();
        if result.matches.len() > 10 || total_matches > 50 {
            ResponseFormatter::create_lightweight_response_for_file_search(&result, summary)
                .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
        } else {
            ResponseFormatter::create_formatted_response(&result, summary)
                .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
        }
    }

    /// Helper method to handle list_languages tool
    async fn handle_list_languages_tool(
        &self,
        request: CallToolRequestParam,
    ) -> Result<CallToolResult, ErrorData> {
        let param: ListLanguagesParam = serde_json::from_value(serde_json::Value::Object(
            request.arguments.unwrap_or_default(),
        ))
        .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
        let result = self.list_languages(param).await.map_err(ErrorData::from)?;
        let summary = ResponseFormatter::format_list_languages_result(&result);
        ResponseFormatter::create_formatted_response(&result, summary)
            .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_search_basic() {
        let service = AstGrepService::new();
        let param = SearchParam::new(
            "function greet() { console.log(\"Hello\"); }",
            "console.log($VAR)",
            "javascript",
        );

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
        let param = SearchParam::new(
            "function greet() { alert(\"Hello\"); }",
            "console.log($VAR)",
            "javascript",
        );

        let result = service.search(param).await.unwrap();
        assert_eq!(result.matches.len(), 0);
    }

    #[tokio::test]
    async fn test_search_invalid_language() {
        let service = AstGrepService::new();
        let param = SearchParam::new(
            "function greet() { console.log(\"Hello\"); }",
            "console.log($VAR)",
            "invalid_language",
        );

        let result = service.search(param).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ServiceError::Internal(_)));
    }

    #[tokio::test]
    async fn test_replace_basic() {
        let service = AstGrepService::new();
        let param = ReplaceParam::new(
            "function oldName() { console.log(\"Hello\"); }",
            "function oldName()",
            "function newName()",
            "javascript",
        );

        let result = service.replace(param).await.unwrap();
        assert!(result.new_code.contains("function newName()"));
        assert!(!result.new_code.contains("function oldName()"));
    }

    #[tokio::test]
    async fn test_replace_with_vars() {
        let service = AstGrepService::new();
        let param = ReplaceParam::new(
            "const x = 5; const y = 10;",
            "const $VAR = $VAL",
            "let $VAR = $VAL",
            "javascript",
        );

        let result = service.replace(param).await.unwrap();
        assert!(result.new_code.contains("let x = 5"));
        assert!(result.new_code.contains("let y = 10"));
        assert!(!result.new_code.contains("const"));
    }

    #[tokio::test]
    async fn test_replace_multiple_occurrences() {
        let service = AstGrepService::new();
        let param = ReplaceParam::new(
            "let a = 1; let b = 2; let c = 3;",
            "let $VAR = $VAL",
            "const $VAR = $VAL",
            "javascript",
        );
        let result = service.replace(param).await.unwrap();
        assert_eq!(result.new_code, "const a = 1; const b = 2; const c = 3;");
    }

    #[tokio::test]
    async fn test_rust_pattern_matching() {
        let service = AstGrepService::new();
        let param = SearchParam::new(
            "fn main() { println!(\"Hello, world!\"); }",
            "println!($VAR)",
            "rust",
        );

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
            additional_rule_dirs: Vec::new(),
            util_dirs: Vec::new(),
            sg_config_path: None,
        };

        let service = AstGrepService::with_config(custom_config);
        assert_eq!(service.config.max_file_size, 1024 * 1024);
        assert_eq!(service.config.max_concurrency, 5);
        assert_eq!(service.config.limit, 10);
    }

    #[tokio::test]
    async fn test_multiple_matches() {
        let service = AstGrepService::new();
        let param = SearchParam::new(
            "console.log(\"Hello\"); console.log(\"World\"); alert(\"test\");",
            "console.log($VAR)",
            "javascript",
        );

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
        let param = SearchParam::new(
            "function test(a, b) { return a + b; } function add(x, y) { return x + y; }",
            "function $NAME($PARAM1, $PARAM2) { return $PARAM1 + $PARAM2; }",
            "javascript",
        );

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
        let param = SearchParam::new("let x = 1;", "let x = 1;", "not_a_real_language");
        let result = service.search(param).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ServiceError::Internal(msg) if msg == "Failed to parse language"));
    }

    #[tokio::test]
    async fn test_pattern_caching() {
        let service = AstGrepService::new();
        let param = SearchParam::new(
            "console.log(\"test\"); console.log(\"another\");",
            "console.log($VAR)",
            "javascript",
        );

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
            .search(SearchParam::new(code, "console.log($VAR)", "javascript"))
            .await
            .unwrap();

        // Add second pattern
        let _ = service
            .search(SearchParam::new(
                code,
                "console.$METHOD($VAR)",
                "javascript",
            ))
            .await
            .unwrap();

        // Cache should have 2 entries
        let (used, capacity) = service.get_cache_stats();
        assert_eq!(used, 2);
        assert_eq!(capacity, 2);

        // Add third pattern - should evict least recently used
        let _ = service
            .search(SearchParam::new(code, "$OBJECT.log($VAR)", "javascript"))
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
            code: "function test() { return 42; }".to_string(),
            language: "javascript".to_string(),
        };

        let result = service.generate_ast(param).await.unwrap();

        // Should contain function declaration
        assert!(result.ast.contains("function_declaration"));
        assert!(result.ast.contains("identifier"));
        assert!(result.ast.contains("number"));
        assert_eq!(result.language, "javascript");
        assert_eq!(result.code_length, 30);
    }

    #[tokio::test]
    async fn test_file_replace_console_log() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AstGrepService::with_config(ServiceConfig {
            root_directories: vec![temp_dir.path().to_path_buf()],
            ..Default::default()
        });
        let file_path = temp_dir.path().join("test_file_replace.js");
        let file_content = r#"
function greet() {
  console.log("Hello");
  console.log("World");
}
"#;
        tokio::fs::write(&file_path, file_content).await.unwrap();

        let path_pattern = "**/test_file_replace.js".to_string();

        // Dry run test
        let dry_run_param = FileReplaceParam {
            path_pattern: path_pattern.clone(),
            pattern: "console.log($VAR)".to_string(),
            replacement: "logger.debug($VAR)".to_string(),
            language: "javascript".to_string(),
            dry_run: true,
            include_samples: true,
            summary_only: true,
            max_file_size: default_max_file_size(),
            max_results: default_max_results_large(),
            cursor: None,
            strictness: None,
            selector: None,
            context: None,
            max_samples: default_max_samples(),
        };

        let dry_run_result = service.file_replace(dry_run_param).await.unwrap();
        assert_eq!(dry_run_result.files_with_changes, 1);
        assert_eq!(dry_run_result.total_changes, 2);
        assert!(!dry_run_result.summary_results.is_empty());
        assert!(!dry_run_result.summary_results[0].sample_changes.is_empty());
        assert_eq!(
            dry_run_result.summary_results[0].sample_changes[0].new_text,
            "logger.debug($VAR)"
        );

        // Actual replacement test
        let replace_param = FileReplaceParam {
            path_pattern: path_pattern.clone(),
            pattern: "console.log($VAR)".to_string(),
            replacement: "logger.debug($VAR)".to_string(),
            language: "javascript".to_string(),
            dry_run: false,
            include_samples: true,
            summary_only: true,
            max_file_size: default_max_file_size(),
            max_results: default_max_results_large(),
            cursor: None,
            strictness: None,
            selector: None,
            context: None,
            max_samples: default_max_samples(),
        };

        let replace_result = service.file_replace(replace_param).await.unwrap();
        assert_eq!(replace_result.files_with_changes, 1);
        assert_eq!(replace_result.total_changes, 2);

        let modified_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(modified_content.contains(r#"logger.debug("Hello")"#));
        assert!(modified_content.contains(r#"logger.debug("World")"#));
        assert!(!modified_content.contains(r#"console.log("Hello")"#));
        assert!(!modified_content.contains(r#"console.log("World")"#));

        // Clean up
        temp_dir.close().unwrap();
    }
}
