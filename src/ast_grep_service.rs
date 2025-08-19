use crate::ast_utils::AstParser;
use crate::config::ServiceConfig;
use crate::errors::ServiceError;
use crate::learning::{
    ExplorePatternParam, GeneratePromptParam, GeneratedPrompt, LearningService, PatternCatalog,
    ValidatePatternParam, ValidationResult,
};
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
        CallToolRequestParam, CallToolResult, ErrorData, GetPromptRequestParam, GetPromptResult,
        Implementation, InitializeResult, ListPromptsResult, ListToolsResult,
        PaginatedRequestParam, Prompt, PromptArgument, PromptMessage, PromptMessageContent,
        PromptMessageRole, PromptsCapability, ProtocolVersion, ServerCapabilities,
    },
    service::{RequestContext, RoleServer},
};
// Removed unused serde imports

#[derive(Clone)]
pub struct AstGrepService {
    #[allow(dead_code)]
    pub(crate) config: ServiceConfig,
    #[allow(dead_code)]
    pub(crate) pattern_cache: Arc<Mutex<LruCache<String, Pattern>>>,
    #[allow(dead_code)]
    pub(crate) pattern_matcher: PatternMatcher,
    #[allow(dead_code)]
    pub(crate) rule_evaluator: RuleEvaluator,
    pub(crate) search_service: SearchService,
    pub(crate) replace_service: ReplaceService,
    pub(crate) rule_service: RuleService,
    pub(crate) learning_service: LearningService,
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
        let learning_service = LearningService::new().unwrap_or_else(|_| {
            // If learning service fails to initialize, create a minimal one
            LearningService::default()
        });

        Self {
            config,
            pattern_cache,
            pattern_matcher,
            rule_evaluator,
            search_service,
            replace_service,
            rule_service,
            learning_service,
        }
    }

    /// Get pattern cache statistics for monitoring and debugging
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

    /// Validate a pattern with learning insights
    #[tracing::instrument(skip(self), fields(pattern = %param.pattern, language = %param.language))]
    pub async fn validate_pattern(
        &self,
        param: ValidatePatternParam,
    ) -> Result<ValidationResult, ServiceError> {
        self.learning_service.validate_pattern(param).await
    }

    /// Explore available patterns in catalog
    #[tracing::instrument(skip(self))]
    pub async fn explore_patterns(
        &self,
        param: ExplorePatternParam,
    ) -> Result<PatternCatalog, ServiceError> {
        self.learning_service.explore_patterns(param).await
    }

    /// Generate LLM prompt for enhanced learning assistance
    pub fn generate_prompt(
        &self,
        param: GeneratePromptParam,
    ) -> Result<GeneratedPrompt, ServiceError> {
        self.learning_service.generate_prompt(param)
    }

    /// Generate quick hint for validation results
    pub fn generate_quick_hint(
        &self,
        validation_result: &ValidationResult,
        pattern: &str,
    ) -> String {
        self.learning_service
            .generate_quick_hint(validation_result, pattern)
    }

    /// Analyze code fragment for extract-function refactoring
    #[tracing::instrument(skip(self), fields(language = %param.language))]
    pub async fn analyze_refactoring(
        &self,
        param: AnalyzeRefactoringParam,
    ) -> Result<AnalyzeRefactoringResult, ServiceError> {
        use crate::refactoring::capture_analysis::CaptureAnalysisEngine;
        
        let engine = CaptureAnalysisEngine::new();
        let analysis = engine.analyze_capture_simple(&param.fragment, &param.context, &param.language)?;
        
        // Convert the analysis to MCP result format
        self.convert_to_mcp_analysis(analysis)
    }

    /// Integrated extract function tool combining analysis and execution
    #[tracing::instrument(skip(self), fields(language = %param.language, function_name = %param.function_name))]
    pub async fn extract_function(
        &self,
        param: ExtractFunctionParam,
    ) -> Result<ExtractFunctionResult, ServiceError> {
        use crate::refactoring::capture_analysis::CaptureAnalysisEngine;
        
        let engine = CaptureAnalysisEngine::new();
        
        // First, analyze the fragment
        let analysis = engine.analyze_capture_simple(&param.fragment, &param.context, &param.language)?;
        let mcp_analysis = self.convert_to_mcp_analysis(analysis.clone())?;
        
        // Generate the extracted function
        let extracted_function = self.generate_extracted_function(
            &param.function_name,
            &param.fragment,
            &analysis,
            &param.language,
        )?;
        
        // Generate the modified context with function call
        let modified_context = self.generate_modified_context(
            &param.context,
            &param.fragment,
            &param.function_name,
            &analysis,
        )?;
        
        Ok(ExtractFunctionResult {
            analysis: mcp_analysis,
            extracted_function,
            modified_context,
            dry_run: param.dry_run.unwrap_or(true),
            success: true,
            messages: vec!["Function extraction completed successfully".to_string()],
        })
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
                prompts: Some(PromptsCapability { list_changed: Some(true) }),
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

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult {
            prompts: vec![
                Prompt {
                    name: "pattern_help".to_string(),
                    description: Some("Get help with AST pattern matching for a specific use case".to_string()),
                    arguments: Some(vec![
                        PromptArgument {
                            name: "use_case".to_string(),
                            description: Some("What you want to achieve (e.g., 'find all console.log statements')".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "language".to_string(),
                            description: Some("Programming language (javascript, python, rust, etc.)".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "complexity".to_string(),
                            description: Some("beginner, intermediate, or advanced".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
                Prompt {
                    name: "pattern_debug".to_string(),
                    description: Some("Debug why an AST pattern isn't matching as expected".to_string()),
                    arguments: Some(vec![
                        PromptArgument {
                            name: "pattern".to_string(),
                            description: Some("The AST pattern that isn't working".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "test_code".to_string(),
                            description: Some("Code you expected the pattern to match".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "language".to_string(),
                            description: Some("Programming language".to_string()),
                            required: Some(true),
                        },
                    ]),
                },
                Prompt {
                    name: "pattern_optimize".to_string(),
                    description: Some("Get suggestions for optimizing an AST pattern".to_string()),
                    arguments: Some(vec![
                        PromptArgument {
                            name: "pattern".to_string(),
                            description: Some("The AST pattern to optimize".to_string()),
                            required: Some(true),
                        },
                        PromptArgument {
                            name: "goal".to_string(),
                            description: Some("What you want to improve (performance, readability, flexibility)".to_string()),
                            required: Some(false),
                        },
                    ]),
                },
            ],
            ..Default::default()
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        match request.name.as_str() {
            "pattern_help" => self.get_pattern_help_prompt(request.arguments),
            "pattern_debug" => self.get_pattern_debug_prompt(request.arguments),
            "pattern_optimize" => self.get_pattern_optimize_prompt(request.arguments),
            _ => Err(ErrorData::invalid_params(
                std::borrow::Cow::Borrowed("Unknown prompt name"),
                None,
            )),
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

    /// Get pattern help prompt
    fn get_pattern_help_prompt(
        &self,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<GetPromptResult, ErrorData> {
        let args = arguments
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing arguments"), None))?;

        let use_case = args
            .get("use_case")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing use_case"), None))?;

        let language = args
            .get("language")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing language"), None))?;

        let complexity = args
            .get("complexity")
            .and_then(|v| v.as_str())
            .unwrap_or("intermediate");

        // Use the learning system to generate helpful content
        let prompt_content = format!(
            "I'll help you create an AST pattern for: {use_case}\n\n\
             Language: {language}\n\
             Complexity Level: {complexity}\n\n\
             Here are some pattern examples and guidance:\n\n\
             1. Basic pattern structure:\n\
             - Use exact text for literal matching\n\
             - Use $VAR to capture single nodes\n\
             - Use $$$ to capture multiple nodes (lists)\n\n\
             2. Common patterns for {language}:\n"
        );

        // Add language-specific examples
        let examples = match language {
            "javascript" | "typescript" => {
                "- Function calls: `functionName($ARG)`\n\
                 - Variable declarations: `const $VAR = $VALUE`\n\
                 - Console logs: `console.log($MSG)`\n\
                 - Async functions: `async function $NAME($PARAMS) { $$$ }`"
            }
            "python" => {
                "- Function definitions: `def $NAME($PARAMS): $$$`\n\
                 - Method calls: `$OBJ.$METHOD($ARGS)`\n\
                 - Print statements: `print($MSG)`\n\
                 - Class definitions: `class $NAME: $$$`"
            }
            "rust" => {
                "- Function definitions: `fn $NAME($PARAMS) -> $RET { $$$ }`\n\
                 - Match expressions: `match $EXPR { $$$ }`\n\
                 - Macro calls: `$MACRO!($ARGS)`\n\
                 - Impl blocks: `impl $TRAIT for $TYPE { $$$ }`"
            }
            _ => {
                "- Function/method patterns vary by language\n\
                 - Check the AST structure with generate_ast tool\n\
                 - Start simple and add complexity gradually"
            }
        };

        let full_prompt = format!(
            "{prompt_content}{examples}\n\n\
             3. Next steps:\n\
             - Use the `validate_pattern` tool to test your pattern\n\
             - Use `explore_patterns` to see more examples\n\
             - Use `generate_ast` to understand the AST structure\n\n\
             Based on your use case '{use_case}', here's a suggested starting pattern:\n"
        );

        // Generate a suggested pattern based on the use case
        let suggested_pattern = self.suggest_pattern_for_use_case(use_case, language);

        Ok(GetPromptResult {
            description: Some(format!("Pattern help for: {use_case}")),
            messages: vec![PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::Text {
                    text: format!("{full_prompt}\n```\n{suggested_pattern}\n```"),
                },
            }],
        })
    }

    /// Get pattern debug prompt
    fn get_pattern_debug_prompt(
        &self,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<GetPromptResult, ErrorData> {
        let args = arguments
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing arguments"), None))?;

        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing pattern"), None))?;

        let test_code = args
            .get("test_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing test_code"), None))?;

        let language = args
            .get("language")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing language"), None))?;

        // Use the validation engine to analyze the pattern
        let validation_param = crate::learning::ValidatePatternParam {
            pattern: pattern.to_string(),
            language: language.to_string(),
            test_code: Some(test_code.to_string()),
            context: None,
        };

        // Run validation synchronously
        let validation_result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(self.learning_service.validate_pattern(validation_param))
        });

        let debug_content = match validation_result {
            Ok(result) => {
                let hint = self.learning_service.generate_quick_hint(&result, pattern);
                format!(
                    "Pattern Debug Analysis:\n\n\
                     Pattern: `{}`\n\
                     Test Code:\n```{}\n{}\n```\n\n\
                     {}\n\n\
                     Analysis:\n\
                     - Valid: {}\n\
                     - Complexity: {:.2}\n\
                     - Metavariables: {}\n\n\
                     {}",
                    pattern,
                    language,
                    test_code,
                    hint,
                    result.is_valid,
                    result.analysis.complexity_score,
                    result.analysis.metavar_usage.len(),
                    if result.is_valid {
                        "✅ Pattern matches successfully!\n\nTry these experiments:\n".to_string()
                            + &result.suggested_experiments.join("\n- ")
                    } else {
                        format!(
                            "❌ Pattern doesn't match. Issues:\n{}\n\nSuggestions:\n{}",
                            result.analysis.potential_issues.join("\n- "),
                            result
                                .learning_insights
                                .iter()
                                .map(|i| format!("- {}", i.actionable_tip))
                                .collect::<Vec<_>>()
                                .join("\n")
                        )
                    }
                )
            }
            Err(e) => format!("Error analyzing pattern: {e}"),
        };

        Ok(GetPromptResult {
            description: Some(format!("Debug analysis for pattern: {pattern}")),
            messages: vec![PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::Text {
                    text: debug_content,
                },
            }],
        })
    }

    /// Get pattern optimize prompt
    fn get_pattern_optimize_prompt(
        &self,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<GetPromptResult, ErrorData> {
        let args = arguments
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing arguments"), None))?;

        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ErrorData::invalid_params(Cow::Borrowed("Missing pattern"), None))?;

        let goal = args
            .get("goal")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let optimize_content = format!(
            "Pattern Optimization Analysis:\n\n\
             Original Pattern: `{pattern}`\n\
             Optimization Goal: {goal}\n\n"
        );

        let suggestions = match goal {
            "performance" => {
                "Performance Optimization:\n\
                 - Use specific literals instead of generic metavariables where possible\n\
                 - Avoid $$$ captures unless necessary (they're more expensive)\n\
                 - Consider using selector constraints for better filtering\n\
                 - Break complex patterns into simpler, targeted ones"
            }
            "readability" => {
                "Readability Improvements:\n\
                 - Use descriptive metavariable names ($FUNCTION_NAME vs $F)\n\
                 - Break complex patterns into multiple simpler patterns\n\
                 - Add comments explaining the pattern's purpose\n\
                 - Consider using YAML rules for complex logic"
            }
            "flexibility" => {
                "Flexibility Enhancements:\n\
                 - Replace literals with metavariables for more matches\n\
                 - Use $$$ for capturing variable-length lists\n\
                 - Consider optional elements with pattern alternatives\n\
                 - Use context patterns for surrounding code flexibility"
            }
            _ => {
                "General Optimization Tips:\n\
                 - Balance specificity with flexibility\n\
                 - Use metavariables strategically\n\
                 - Consider maintenance and future changes\n\
                 - Test with diverse code samples"
            }
        };

        let optimized_pattern = self.suggest_optimized_pattern(pattern, goal);

        Ok(GetPromptResult {
            description: Some(format!("Optimization suggestions for: {pattern}")),
            messages: vec![PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::Text {
                    text: format!(
                        "{optimize_content}{suggestions}\n\n\
                         Suggested Optimized Pattern:\n```\n{optimized_pattern}\n```\n\n\
                         Additional Tips:\n\
                         - Use `validate_pattern` to test the optimized version\n\
                         - Compare match results between original and optimized\n\
                         - Consider the trade-offs for your specific use case"
                    ),
                },
            }],
        })
    }

    /// Suggest a pattern based on use case description
    fn suggest_pattern_for_use_case(&self, use_case: &str, language: &str) -> String {
        let use_case_lower = use_case.to_lowercase();

        match language {
            "javascript" | "typescript" if use_case_lower.contains("console.log") => {
                "console.log($MESSAGE)"
            }
            "javascript" | "typescript" if use_case_lower.contains("function") => {
                "function $NAME($PARAMS) { $$$ }"
            }
            "python" if use_case_lower.contains("print") => "print($MESSAGE)",
            "python" if use_case_lower.contains("function") || use_case_lower.contains("def") => {
                "def $NAME($PARAMS):\n    $$$"
            }
            "rust" if use_case_lower.contains("function") => "fn $NAME($PARAMS) -> $RET { $$$ }",
            _ => "$PATTERN",
        }
        .to_string()
    }

    /// Suggest an optimized version of a pattern
    fn suggest_optimized_pattern(&self, pattern: &str, goal: &str) -> String {
        match goal {
            "performance" if pattern.contains("$$$") => pattern.replace("$$$", "$SPECIFIC_NODE"),
            "readability" => {
                // Make metavariable names more descriptive
                pattern
                    .replace("$VAR", "$VARIABLE_NAME")
                    .replace("$ARG", "$ARGUMENT")
                    .replace("$MSG", "$MESSAGE")
            }
            "flexibility" => {
                // Add more metavariables
                if !pattern.contains("$") {
                    format!("$FLEXIBLE_{pattern}")
                } else {
                    pattern.to_string()
                }
            }
            _ => pattern.to_string(),
        }
    }

    /// Convert internal CaptureAnalysis to MCP AnalyzeRefactoringResult format
    fn convert_to_mcp_analysis(
        &self,
        analysis: crate::refactoring::capture_analysis::CaptureAnalysis,
    ) -> Result<AnalyzeRefactoringResult, ServiceError> {
        use crate::types::{VariableUsageInfo, SideEffectInfo, ReturnValueInfo, ReturnStrategyInfo, FunctionSignatureInfo, ScopeAnalysisInfo};
        use std::collections::HashMap;
        
        let external_reads: Vec<VariableUsageInfo> = analysis.external_reads
            .into_iter()
            .map(|var| VariableUsageInfo {
                name: var.name,
                var_type: var.var_type,
                usage_type: format!("{:?}", var.usage_type),
                scope_level: var.scope_level,
                first_usage_line: var.first_usage_line,
            })
            .collect();
        
        let external_writes: Vec<VariableUsageInfo> = analysis.external_writes
            .into_iter()
            .map(|var| VariableUsageInfo {
                name: var.name,
                var_type: var.var_type,
                usage_type: format!("{:?}", var.usage_type),
                scope_level: var.scope_level,
                first_usage_line: var.first_usage_line,
            })
            .collect();

        let internal_declarations: Vec<VariableUsageInfo> = analysis.internal_declarations
            .into_iter()
            .map(|var| VariableUsageInfo {
                name: var.name,
                var_type: var.var_type,
                usage_type: format!("{:?}", var.usage_type),
                scope_level: var.scope_level,
                first_usage_line: var.first_usage_line,
            })
            .collect();

        let side_effects: Vec<SideEffectInfo> = analysis.side_effects
            .into_iter()
            .map(|effect| {
                let (effect_type, description, target, details) = match effect {
                    crate::refactoring::capture_analysis::SideEffect::FunctionCall { name, args } => {
                        let mut details = HashMap::new();
                        details.insert("args".to_string(), args.join(", "));
                        ("function_call".to_string(), format!("Function call to {name}"), Some(name), details)
                    },
                    crate::refactoring::capture_analysis::SideEffect::GlobalMutation { variable } => {
                        ("global_mutation".to_string(), format!("Global variable mutation: {variable}"), Some(variable), HashMap::new())
                    },
                    crate::refactoring::capture_analysis::SideEffect::IOOperation { operation_type } => {
                        ("io_operation".to_string(), format!("I/O operation: {operation_type}"), None, HashMap::new())
                    },
                    crate::refactoring::capture_analysis::SideEffect::StateModification { target } => {
                        ("state_modification".to_string(), format!("State modification: {target}"), Some(target), HashMap::new())
                    },
                    crate::refactoring::capture_analysis::SideEffect::AsyncOperation { operation_type, target } => {
                        ("async_operation".to_string(), format!("Async operation: {operation_type}"), target, HashMap::new())
                    },
                    crate::refactoring::capture_analysis::SideEffect::DOMManipulation { element, action } => {
                        let mut details = HashMap::new();
                        details.insert("action".to_string(), action);
                        ("dom_manipulation".to_string(), format!("DOM manipulation on {element}"), Some(element), details)
                    },
                    crate::refactoring::capture_analysis::SideEffect::NetworkOperation { url, method } => {
                        let mut details = HashMap::new();
                        details.insert("method".to_string(), method);
                        ("network_operation".to_string(), format!("Network operation: {url}"), Some(url), details)
                    },
                };
                
                SideEffectInfo {
                    effect_type,
                    description,
                    target,
                    details,
                }
            })
            .collect();

        let return_values: Vec<ReturnValueInfo> = analysis.return_values
            .into_iter()
            .map(|ret| ReturnValueInfo {
                expression: ret.expression,
                inferred_type: ret.inferred_type,
                is_mutation_result: ret.is_mutation_result,
            })
            .collect();

        let suggested_return_strategy = analysis.suggested_return.map(|strategy| {
            match strategy {
                crate::refactoring::capture_analysis::ReturnStrategy::Single { expression, var_type } => {
                    ReturnStrategyInfo {
                        strategy_type: "single".to_string(),
                        description: "Return a single value".to_string(),
                        expression: Some(expression),
                        values: None,
                        modified_params: None,
                        return_type: var_type,
                    }
                },
                crate::refactoring::capture_analysis::ReturnStrategy::Multiple { values } => {
                    ReturnStrategyInfo {
                        strategy_type: "multiple".to_string(),
                        description: "Return multiple values".to_string(),
                        expression: None,
                        values: Some(values),
                        modified_params: None,
                        return_type: None,
                    }
                },
                crate::refactoring::capture_analysis::ReturnStrategy::InPlace { modified_params } => {
                    ReturnStrategyInfo {
                        strategy_type: "in_place".to_string(),
                        description: "Modify parameters in place".to_string(),
                        expression: None,
                        values: None,
                        modified_params: Some(modified_params),
                        return_type: None,
                    }
                },
                crate::refactoring::capture_analysis::ReturnStrategy::Void => {
                    ReturnStrategyInfo {
                        strategy_type: "void".to_string(),
                        description: "No return value needed".to_string(),
                        expression: None,
                        values: None,
                        modified_params: None,
                        return_type: Some("void".to_string()),
                    }
                },
            }
        });

        let parameters: Vec<String> = analysis.suggested_parameters
            .into_iter()
            .map(|param| if let Some(param_type) = param.param_type {
                format!("{}: {}", param.name, param_type)
            } else {
                param.name
            })
            .collect();

        let is_pure = side_effects.is_empty() && external_writes.is_empty();
        
        let suggested_signature = FunctionSignatureInfo {
            name: "extracted_function".to_string(),
            parameters: parameters.clone(),
            return_info: suggested_return_strategy
                .as_ref()
                .map(|s| s.strategy_type.clone())
                .unwrap_or_else(|| "void".to_string()),
            full_signature: format!(
                "function extracted_function({}): {}",
                parameters.join(", "),
                suggested_return_strategy
                    .as_ref()
                    .and_then(|s| s.return_type.as_ref())
                    .unwrap_or(&"void".to_string())
            ),
            is_pure,
        };

        let scope_info = ScopeAnalysisInfo {
            current_scope_type: "unknown".to_string(), // TODO: extract from analysis
            scope_depth: 0, // TODO: extract from analysis
            crosses_boundaries: false, // TODO: extract from analysis
            violations: vec![], // TODO: extract from analysis
            instance_members: vec![], // TODO: extract from analysis
        };

        Ok(AnalyzeRefactoringResult {
            external_reads,
            external_writes,
            internal_declarations,
            return_values,
            suggested_return_strategy,
            side_effects,
            suggested_signature,
            scope_info,
        })
    }

    /// Generate the extracted function code
    fn generate_extracted_function(
        &self,
        function_name: &str,
        fragment: &str,
        analysis: &crate::refactoring::capture_analysis::CaptureAnalysis,
        language: &str,
    ) -> Result<String, ServiceError> {
        let external_reads: Vec<String> = analysis.external_reads
            .iter()
            .map(|var| var.name.clone())
            .collect();
        
        let external_writes: Vec<String> = analysis.external_writes
            .iter()
            .map(|var| var.name.clone())
            .collect();

        // Generate function signature based on language
        let function_code = match language {
            "javascript" | "typescript" => {
                let params = external_reads.join(", ");
                let return_statement = if !external_writes.is_empty() {
                    if external_writes.len() == 1 {
                        format!("\n    return {};", external_writes[0])
                    } else {
                        format!("\n    return {{ {} }};", external_writes.join(", "))
                    }
                } else {
                    String::new()
                };
                
                format!("function {function_name}({params}) {{\n    {fragment}{return_statement}\n}}")
            },
            "python" => {
                let params = external_reads.join(", ");
                let return_statement = if !external_writes.is_empty() {
                    if external_writes.len() == 1 {
                        format!("\n    return {}", external_writes[0])
                    } else {
                        format!("\n    return {}", external_writes.join(", "))
                    }
                } else {
                    String::new()
                };
                
                format!("def {function_name}({params}):\n    {}{return_statement}", 
                       fragment.replace('\n', "\n    "))
            },
            "rust" => {
                let params: Vec<String> = external_reads.iter()
                    .map(|param| format!("{param}: &str")) // Simple type assumption
                    .collect();
                let params_str = params.join(", ");
                
                let return_type = if external_writes.is_empty() {
                    String::new()
                } else if external_writes.len() == 1 {
                    " -> String".to_string() // Simple return type
                } else {
                    format!(" -> ({})", external_writes.iter().map(|_| "String").collect::<Vec<_>>().join(", "))
                };
                
                let return_statement = if !external_writes.is_empty() {
                    if external_writes.len() == 1 {
                        format!("\n    {}", external_writes[0])
                    } else {
                        format!("\n    ({})", external_writes.join(", "))
                    }
                } else {
                    String::new()
                };
                
                format!("fn {function_name}({params_str}){return_type} {{\n    {fragment}{return_statement}\n}}")
            },
            _ => {
                // Generic format
                let params = external_reads.join(", ");
                format!("{function_name}({params}) {{\n    {fragment}\n}}")
            }
        };

        Ok(function_code)
    }

    /// Generate the modified context with function call
    fn generate_modified_context(
        &self,
        context: &str,
        fragment: &str,
        function_name: &str,
        analysis: &crate::refactoring::capture_analysis::CaptureAnalysis,
    ) -> Result<String, ServiceError> {
        let external_reads: Vec<String> = analysis.external_reads
            .iter()
            .map(|var| var.name.clone())
            .collect();
        
        let external_writes: Vec<String> = analysis.external_writes
            .iter()
            .map(|var| var.name.clone())
            .collect();

        // Generate function call
        let args = external_reads.join(", ");
        let function_call = if external_writes.is_empty() {
            format!("{function_name}({args});")
        } else if external_writes.len() == 1 {
            format!("{} = {function_name}({args});", external_writes[0])
        } else {
            // Handle multiple returns based on language conventions
            format!("// TODO: Handle multiple return values: {}\n    {function_name}({args});", external_writes.join(", "))
        };

        // Replace the fragment with the function call
        let modified_context = context.replace(fragment, &function_call);
        
        Ok(modified_context)
    }

    /// Apply structured refactorings
    pub async fn refactor(
        &self,
        param: crate::refactoring::RefactoringRequest,
    ) -> Result<crate::refactoring::RefactoringResponse, ServiceError> {
        use crate::refactoring::RefactoringService;
        use std::sync::Arc;
        
        let service = RefactoringService::new(
            Arc::new(self.search_service.clone()),
            Arc::new(self.replace_service.clone())
        ).map_err(|e| ServiceError::Internal(e.to_string()))?;
        
        service.refactor(param).await
            .map_err(|e| ServiceError::AstAnalysisError {
                message: e.to_string(),
                code: "refactoring_failed".to_string(),
                language: "unknown".to_string(),
                ast_structure: String::new(),
                node_kinds: vec![],
            })
    }

    /// Validate refactoring against test code
    pub async fn validate_refactoring(
        &self,
        param: crate::refactoring::ValidateRefactoringRequest,
    ) -> Result<crate::refactoring::ValidateRefactoringResponse, ServiceError> {
        use crate::refactoring::RefactoringService;
        use std::sync::Arc;
        
        let service = RefactoringService::new(
            Arc::new(self.search_service.clone()),
            Arc::new(self.replace_service.clone())
        ).map_err(|e| ServiceError::Internal(e.to_string()))?;
        
        service.validate_refactoring(param).await
            .map_err(|e| ServiceError::AstAnalysisError {
                message: e.to_string(),
                code: "refactoring_validation_failed".to_string(),
                language: "unknown".to_string(),
                ast_structure: String::new(),
                node_kinds: vec![],
            })
    }

    /// List available refactorings
    pub async fn list_refactorings(
        &self,
        _filters: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<Vec<crate::refactoring::RefactoringInfo>, ServiceError> {
        use crate::refactoring::RefactoringService;
        use std::sync::Arc;
        
        let service = RefactoringService::new(
            Arc::new(self.search_service.clone()),
            Arc::new(self.replace_service.clone())
        ).map_err(|e| ServiceError::Internal(e.to_string()))?;
        
        service.list_refactorings().await
            .map_err(|e| ServiceError::AstAnalysisError {
                message: e.to_string(),
                code: "list_refactorings_failed".to_string(),
                language: "unknown".to_string(),
                ast_structure: String::new(),
                node_kinds: vec![],
            })
    }

    /// Get detailed refactoring information
    pub async fn get_refactoring_info(
        &self,
        refactoring_id: &str,
    ) -> Result<crate::refactoring::RefactoringDetails, ServiceError> {
        use crate::refactoring::RefactoringService;
        use std::sync::Arc;
        
        let service = RefactoringService::new(
            Arc::new(self.search_service.clone()),
            Arc::new(self.replace_service.clone())
        ).map_err(|e| ServiceError::Internal(e.to_string()))?;
        
        service.get_refactoring_info(refactoring_id).await
            .map_err(|e| ServiceError::AstAnalysisError {
                message: e.to_string(),
                code: "get_refactoring_info_failed".to_string(),
                language: "unknown".to_string(),
                ast_structure: String::new(),
                node_kinds: vec![],
            })
    }
}

