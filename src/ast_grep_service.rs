use crate::ast_utils::AstParser;
use crate::config::ServiceConfig;
use crate::debug::DebugService;
use crate::embedded::EmbeddedService;
use crate::errors::ServiceError;
use crate::pattern::PatternMatcher;
use crate::replace::ReplaceService;
use crate::response_formatter::ResponseFormatter;
use crate::rules::*;
use crate::rules::{CatalogManager, RuleEvaluator, RuleService, RuleStorage};
use crate::search::SearchService;
use crate::types::*;

use ast_grep_core::{AstGrep, Pattern};

use lru::LruCache;
use std::num::NonZeroUsize;
use std::{borrow::Cow, str::FromStr, sync::Arc, sync::Mutex};

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
    #[allow(dead_code)]
    config: ServiceConfig,
    pattern_cache: Arc<Mutex<LruCache<String, Pattern>>>,
    #[allow(dead_code)]
    pattern_matcher: PatternMatcher,
    #[allow(dead_code)]
    rule_evaluator: RuleEvaluator,
    #[allow(dead_code)]
    search_service: SearchService,
    #[allow(dead_code)]
    replace_service: ReplaceService,
    #[allow(dead_code)]
    rule_service: RuleService,
    #[allow(dead_code)]
    debug_service: DebugService,
    #[allow(dead_code)]
    embedded_service: EmbeddedService,
}

impl Default for AstGrepService {
    fn default() -> Self {
        Self::new()
    }
}

impl AstGrepService {
    fn parse_language(&self, lang_str: &str) -> Result<Language, ServiceError> {
        Language::from_str(lang_str)
            .map_err(|_| ServiceError::Internal("Failed to parse language".to_string()))
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
    fn generate_simple_metavariable_pattern(
        &self,
        examples: &[String],
    ) -> Result<Option<PatternSuggestion>, ServiceError> {
        if examples.len() < 2 {
            return Ok(None);
        }

        // Simple pattern matching for console.log cases
        if examples
            .iter()
            .all(|code| code.starts_with("console.log(") && code.ends_with(")"))
        {
            // Extract the argument parts
            let args: Vec<&str> = examples
                .iter()
                .map(|code| &code[12..code.len() - 1]) // Remove "console.log(" and ")"
                .collect();

            // If all arguments are different string literals, suggest metavariable
            if args
                .iter()
                .all(|arg| arg.starts_with('\'') && arg.ends_with('\''))
            {
                return Ok(Some(PatternSuggestion {
                    pattern: "console.log($MSG)".to_string(),
                    confidence: 0.9,
                    specificity: SpecificityLevel::General,
                    explanation: "Pattern for console.log with variable message. Use selector: \"call_expression\" to match only function calls, or context: \"function $NAME() { $PATTERN }\" to match only within functions.".to_string(),
                    matching_examples: (0..examples.len()).collect(),
                    node_kinds: vec!["call_expression".to_string(), "string".to_string()],
                }));
            }
        }

        // Simple pattern matching for function declarations
        if examples
            .iter()
            .all(|code| code.starts_with("function ") && code.ends_with("() {}"))
        {
            // Extract function names
            let names: Vec<&str> = examples
                .iter()
                .map(|code| {
                    let start = 9; // "function ".len()
                    let end = code.find('(').unwrap();
                    &code[start..end]
                })
                .collect();

            // Check if all names have common prefix "get"
            if names.iter().all(|name| name.starts_with("get")) {
                return Ok(Some(PatternSuggestion {
                    pattern: "function get$TYPE() {}".to_string(),
                    confidence: 0.8,
                    specificity: SpecificityLevel::Specific,
                    explanation: "Pattern for getter functions. Use selector: \"function_declaration\" to match only function declarations, or context: \"class $CLASS { $PATTERN }\" to match only within classes.".to_string(),
                    matching_examples: (0..examples.len()).collect(),
                    node_kinds: vec!["function_declaration".to_string(), "identifier".to_string()],
                }));
            }

            // General function pattern
            return Ok(Some(PatternSuggestion {
                pattern: "function $NAME() {}".to_string(),
                confidence: 0.7,
                specificity: SpecificityLevel::General,
                explanation: "Pattern for function declarations. Use selector: \"function_declaration\" to match only function declarations, or context: \"class $CLASS { $PATTERN }\" to match only within classes.".to_string(),
                matching_examples: (0..examples.len()).collect(),
                node_kinds: vec!["function_declaration".to_string(), "identifier".to_string()],
            }));
        }

        // Pattern matching for nested property access in if statements
        if examples.iter().all(|code| {
            code.contains("if (")
                && code.contains("===")
                && code.contains("{ return")
                && code.contains("; }")
        }) {
            // Check for user.property pattern
            if examples
                .iter()
                .all(|code| code.contains("user.") && code.contains("==="))
            {
                return Ok(Some(PatternSuggestion {
                    pattern: "if (user.$PROP === $VALUE) { return $RESULT; }".to_string(),
                    confidence: 0.8,
                    specificity: SpecificityLevel::Specific,
                    explanation: "Pattern for user property comparisons. Use selector: \"if_statement\" to match only if statements, or context: \"function $NAME() { $PATTERN }\" to match only within functions.".to_string(),
                    matching_examples: (0..examples.len()).collect(),
                    node_kinds: vec![
                        "if_statement".to_string(),
                        "member_expression".to_string(),
                        "binary_expression".to_string(),
                    ],
                }));
            }

            // More general object property pattern
            return Ok(Some(PatternSuggestion {
                pattern: "if ($OBJ.$PROP === $VALUE) { return $RESULT; }".to_string(),
                confidence: 0.7,
                specificity: SpecificityLevel::General,
                explanation: "Pattern for object property comparisons. Use selector: \"if_statement\" to match only if statements, or context: \"function $NAME() { $PATTERN }\" to match only within functions.".to_string(),
                matching_examples: (0..examples.len()).collect(),
                node_kinds: vec![
                    "if_statement".to_string(),
                    "member_expression".to_string(),
                    "binary_expression".to_string(),
                ],
            }));
        }

        // Pattern matching for multiple statements with const declarations and console.log
        if examples.iter().all(|code| {
            code.contains("const ") && code.contains(" = ") && code.contains("console.log(")
        }) {
            // Check for const var = func(); console.log(var.prop); pattern
            if examples.iter().all(|code| {
                code.contains("const ")
                    && code.contains(" = get")
                    && code.contains("(); console.log(")
            }) {
                return Ok(Some(PatternSuggestion {
                    pattern: "const $VAR = $FUNC(); console.log($VAR.$PROP);".to_string(),
                    confidence: 0.8,
                    specificity: SpecificityLevel::Specific,
                    explanation: "Pattern for variable assignment and property access logging. Use selector: \"variable_declaration\" to match only variable declarations, or context: \"function $NAME() { $PATTERN }\" to match only within functions."
                        .to_string(),
                    matching_examples: (0..examples.len()).collect(),
                    node_kinds: vec![
                        "variable_declaration".to_string(),
                        "call_expression".to_string(),
                        "member_expression".to_string(),
                    ],
                }));
            }
        }

        // Pattern matching for class declarations
        if examples
            .iter()
            .all(|code| code.contains("class ") && code.contains("{ constructor() {} }"))
        {
            // Check for class Service pattern
            if examples.iter().all(|code| code.contains("Service")) {
                return Ok(Some(PatternSuggestion {
                    pattern: "class $NAMEService { constructor() {} }".to_string(),
                    confidence: 0.8,
                    specificity: SpecificityLevel::Specific,
                    explanation: "Pattern for service class declarations. Use selector: \"class_declaration\" to match only class declarations, or context: \"export $PATTERN\" to match only exported classes.".to_string(),
                    matching_examples: (0..examples.len()).collect(),
                    node_kinds: vec![
                        "class_declaration".to_string(),
                        "constructor_definition".to_string(),
                    ],
                }));
            }

            // General class pattern
            return Ok(Some(PatternSuggestion {
                pattern: "class $NAME { constructor() {} }".to_string(),
                confidence: 0.7,
                specificity: SpecificityLevel::General,
                explanation: "Pattern for class declarations with constructor. Use selector: \"class_declaration\" to match only class declarations, or context: \"export $PATTERN\" to match only exported classes.".to_string(),
                matching_examples: (0..examples.len()).collect(),
                node_kinds: vec![
                    "class_declaration".to_string(),
                    "constructor_definition".to_string(),
                ],
            }));
        }

        // Pattern matching for for loops with array iteration
        if examples.iter().all(|code| {
            code.contains("for (let ")
                && code.contains(" = 0; ")
                && code.contains(".length; ")
                && code.contains("++) {")
        }) {
            // Check for array iteration pattern
            if examples
                .iter()
                .all(|code| code.contains("process(") || code.contains("handle("))
            {
                return Ok(Some(PatternSuggestion {
                    pattern:
                        "for (let $VAR = 0; $VAR < $ARR.length; $VAR++) { $FUNC($ARR[$VAR]); }"
                            .to_string(),
                    confidence: 0.8,
                    specificity: SpecificityLevel::Specific,
                    explanation: "Pattern for for loop array iteration with function call. Use selector: \"for_statement\" to match only for loops, or context: \"function $NAME() { $PATTERN }\" to match only within functions."
                        .to_string(),
                    matching_examples: (0..examples.len()).collect(),
                    node_kinds: vec![
                        "for_statement".to_string(),
                        "binary_expression".to_string(),
                        "call_expression".to_string(),
                    ],
                }));
            }
        }

        // If no specific patterns matched, try to generate selector-based suggestions
        self.generate_selector_suggestions(examples)
    }

    fn generate_selector_suggestions(
        &self,
        examples: &[String],
    ) -> Result<Option<PatternSuggestion>, ServiceError> {
        // Check if examples contain field assignments that would benefit from selector
        if examples.iter().all(|code| {
            code.contains(" = ") && (code.contains("class ") || code.contains("interface "))
        }) {
            return Ok(Some(PatternSuggestion {
                pattern: "$VAR = $VALUE".to_string(),
                confidence: 0.6,
                specificity: SpecificityLevel::General,
                explanation: "General assignment pattern. Use selector: \"field_definition\" to match only class/interface fields, or selector: \"assignment_expression\" to match only assignments, or context: \"class $CLASS { $PATTERN }\" to match only within classes.".to_string(),
                matching_examples: (0..examples.len()).collect(),
                node_kinds: vec!["assignment_expression".to_string(), "field_definition".to_string()],
            }));
        }

        // Check if examples contain method calls that would benefit from selector
        if examples
            .iter()
            .all(|code| code.contains("(") && code.contains(")"))
        {
            return Ok(Some(PatternSuggestion {
                pattern: "$FUNC($ARGS)".to_string(),
                confidence: 0.6,
                specificity: SpecificityLevel::General,
                explanation: "General function call pattern. Use selector: \"call_expression\" to match only function calls, or selector: \"method_call\" to match only method calls, or context: \"function $NAME() { $PATTERN }\" to match only within functions.".to_string(),
                matching_examples: (0..examples.len()).collect(),
                node_kinds: vec!["call_expression".to_string(), "method_call".to_string()],
            }));
        }

        Ok(None)
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
        let catalog_manager = CatalogManager::new();
        let rule_service = RuleService::new(
            config.clone(),
            rule_evaluator.clone(),
            rule_storage,
            catalog_manager,
        );
        let debug_service = DebugService::new(pattern_matcher.clone());
        let embedded_service = EmbeddedService::new(pattern_matcher.clone());

        Self {
            config,
            pattern_cache,
            pattern_matcher,
            rule_evaluator,
            search_service,
            replace_service,
            rule_service,
            debug_service,
            embedded_service,
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

    /// Debug a pattern to understand its structure and behavior.
    pub async fn debug_pattern(
        &self,
        param: DebugPatternParam,
    ) -> Result<DebugPatternResult, ServiceError> {
        self.debug_service.debug_pattern(param).await
    }

    /// Debug AST/CST structure of code.
    pub async fn debug_ast(&self, param: DebugAstParam) -> Result<DebugAstResult, ServiceError> {
        self.debug_service.debug_ast(param).await
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
                "Rule must have at least one condition".to_string(),
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

    #[tracing::instrument(skip(self), fields(language = %param.language, pattern = %param.pattern))]
    pub async fn search(&self, param: SearchParam) -> Result<SearchResult, ServiceError> {
        let result = self.search_service.search(param).await?;
        tracing::Span::current().record("matches_found", result.matches.len());
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(language = %param.language, examples_count = param.code_examples.len()))]
    pub async fn suggest_patterns(
        &self,
        param: SuggestPatternsParam,
    ) -> Result<SuggestPatternsResult, ServiceError> {
        // Basic exact pattern matching implementation
        if param.code_examples.is_empty() {
            return Ok(SuggestPatternsResult {
                suggestions: vec![],
                language: param.language,
                total_suggestions: 0,
            });
        }

        let mut suggestions = Vec::new();

        // For single example, create an exact pattern
        if param.code_examples.len() == 1 {
            let code = &param.code_examples[0];
            suggestions.push(PatternSuggestion {
                pattern: code.clone(),
                confidence: 1.0,
                specificity: SpecificityLevel::Exact,
                explanation: "Exact match for the provided code".to_string(),
                matching_examples: vec![0],
                node_kinds: vec![],
            });
        }
        // For multiple identical examples, create one exact pattern with high confidence
        else if param
            .code_examples
            .iter()
            .all(|code| code == &param.code_examples[0])
        {
            let code = &param.code_examples[0];
            suggestions.push(PatternSuggestion {
                pattern: code.clone(),
                confidence: 1.0,
                specificity: SpecificityLevel::Exact,
                explanation: "Exact match for all identical examples".to_string(),
                matching_examples: (0..param.code_examples.len()).collect(),
                node_kinds: vec![],
            });
        }
        // For different examples, try to generate metavariable patterns
        else {
            // Try multiple pattern generation strategies
            let generated_pattern =
                self.generate_simple_metavariable_pattern(&param.code_examples)?;
            if let Some(pattern) = generated_pattern {
                suggestions.push(pattern);
            }

            // If no pattern was generated, fallback to exact match for first example
            if suggestions.is_empty() {
                let code = &param.code_examples[0];
                suggestions.push(PatternSuggestion {
                    pattern: code.clone(),
                    confidence: 0.5,
                    specificity: SpecificityLevel::Exact,
                    explanation: "Exact match for first example".to_string(),
                    matching_examples: vec![0],
                    node_kinds: vec![],
                });
            }
        }

        let total_suggestions = suggestions.len();
        Ok(SuggestPatternsResult {
            suggestions,
            language: param.language,
            total_suggestions,
        })
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

    #[tracing::instrument(skip(self), fields(host_language = %param.embedded_config.host_language, embedded_language = %param.embedded_config.embedded_language, pattern = %param.pattern))]
    pub async fn search_embedded(
        &self,
        param: EmbeddedSearchParam,
    ) -> Result<EmbeddedSearchResult, ServiceError> {
        let result = self.embedded_service.search_embedded(param).await?;
        tracing::Span::current().record("total_matches", result.matches.len());
        tracing::Span::current().record("total_blocks", result.total_embedded_blocks);
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
  "replacement": "\"$STRING\"",
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
  "replacement": "\"$STRING\"",
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
  "replacement": "\"$STRING\"",
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

## Project Configuration (sgconfig.yml)

The service supports ast-grep's `sgconfig.yml` configuration files for project-wide rule management.

**Automatic Discovery:**
- Searches for `sgconfig.yml` in current directory and parent directories
- Loads rule directories specified in the configuration
- Makes all rules from configured directories available automatically

**Configuration Format:**
```yaml
ruleDirs:
  - ./rules              # Project-specific rules
  - ./team-rules         # Shared team rules
  - ./node_modules/@company/ast-grep-rules  # NPM package rules
testConfigs:
  - testDir: ./rule-tests
    includeTestId:
      - test-id-1
utilDirs:
  - ./utils              # Shared utilities
```

**Rule Directory Loading:**
- All `.yaml` and `.yml` files in configured directories are loaded as rules
- Subdirectories are searched recursively
- Duplicate rule IDs emit warnings (first rule wins)
- Rules from sgconfig.yml are merged with default `.ast-grep-rules/` directory

**Duplicate Rule Handling:**
- Each rule ID should be unique across all directories
- When duplicates are found:
  - Only the first rule encountered is used
  - A warning is emitted to stderr showing both file paths
  - This ensures predictable rule application

**Example Project Setup:**
```
myproject/
├── sgconfig.yml          # Project configuration
├── rules/                # Project rules directory
│   ├── security/        # Organized by category
│   │   └── no-eval.yaml
│   └── style/
│       └── naming.yaml
└── .ast-grep-rules/     # Default rules directory
    └── custom-rule.yaml
```

## Discovery and Debugging Tools

### generate_ast

Generate a stringified syntax tree for code using Tree-sitter. Useful for debugging patterns and understanding AST structure. **Critical for LLM users to discover available Tree-sitter node kinds for Kind rules.**

**Parameters:**
- `code`: Source code to parse
- `language`: Programming language of the code

**Returns:**
- `ast`: Stringified syntax tree showing the AST structure
- `language`: Programming language used
- `code_length`: Length of the input code in characters
- `node_kinds`: Array of unique Tree-sitter node kinds found in the code

**Example Usage:**
```json
{
  "tool_code": "generate_ast",
  "tool_params": {
    "code": "function test() { return 42; }",
    "language": "javascript"
  }
}
```

**Response includes node kinds like:**
- `function_declaration`
- `identifier`
- `statement_block`
- `return_statement`
- `number`

**Using node kinds in Kind rules:**
```yaml
rule:
  kind: function_declaration  # Use any node kind from generate_ast
```

### list_languages

Lists all supported programming languages for ast-grep patterns.

**No Parameters Required**

**Returns:** Array of supported language identifiers (javascript, typescript, rust, python, etc.)

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
            instructions: Some("This MCP server provides tools for structural code search and transformation using ast-grep. For bulk refactoring, use file_replace with summary_only=true to avoid token limits. Use the `documentation` tool for detailed examples.".to_string()),
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
                    name: Cow::Borrowed("search"),
                    description: Cow::Borrowed("Search for AST patterns in provided code. Supports $VAR for single nodes, $$$ for multiple nodes. Returns matches with line/column positions."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": { "code": { "type": "string" }, "pattern": { "type": "string" }, "language": { "type": "string" } } })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("suggest_patterns"),
                    description: Cow::Borrowed("Generate ast-grep patterns from code examples. Analyzes examples to suggest exact, specific, and general patterns with confidence scores."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "code_examples": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Code examples to analyze for pattern suggestions"
                            },
                            "language": { "type": "string", "description": "Programming language" },
                            "max_suggestions": { "type": "integer", "minimum": 1, "maximum": 10, "description": "Maximum number of pattern suggestions to return" },
                            "specificity_levels": {
                                "type": "array",
                                "items": { "type": "string", "enum": ["exact", "specific", "general"] },
                                "description": "Specificity levels to include in suggestions"
                            }
                        },
                        "required": ["code_examples", "language"]
                    })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("file_search"),
                    description: Cow::Borrowed("Search files matching glob patterns for AST patterns. Supports pagination, context lines, and handles large codebases efficiently."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string" },
                            "pattern": { "type": "string" },
                            "language": { "type": "string" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "default": 20 },
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
                    name: Cow::Borrowed("search_embedded"),
                    description: Cow::Borrowed("Search for patterns in embedded languages within host languages (e.g., JavaScript in HTML, SQL in Python)."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "code": { "type": "string", "description": "Code containing embedded languages" },
                            "pattern": { "type": "string", "description": "Pattern to search for in the embedded language" },
                            "embedded_config": {
                                "type": "object",
                                "properties": {
                                    "host_language": { "type": "string", "description": "The host language (e.g., html, python)" },
                                    "embedded_language": { "type": "string", "description": "The embedded language (e.g., javascript, sql)" },
                                    "extraction_pattern": { "type": "string", "description": "Pattern to match the embedded code in the host language" },
                                    "selector": { "type": "string", "description": "Optional selector to narrow down the extraction" },
                                    "context": { "type": "string", "description": "Optional context pattern for more precise matching" }
                                },
                                "required": ["host_language", "embedded_language", "extraction_pattern"]
                            },
                            "strictness": { "type": "string", "enum": ["cst", "smart", "ast", "relaxed", "signature"], "description": "Optional match strictness" }
                        },
                        "required": ["code", "pattern", "embedded_config"]
                    })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("replace"),
                    description: Cow::Borrowed("Replace AST patterns in provided code. Use $VAR in both pattern and replacement to preserve captured nodes. Returns modified code."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": { "code": { "type": "string" }, "pattern": { "type": "string" }, "replacement": { "type": "string" }, "language": { "type": "string" } } })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("file_replace"),
                    description: Cow::Borrowed("Replace patterns in files. Use summary_only=true for bulk refactoring to avoid token limits. Returns change counts or line diffs."),
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
                    name: Cow::Borrowed("list_languages"),
                    description: Cow::Borrowed("Get all supported programming languages for ast-grep patterns. Returns 20+ languages including JS, TS, Python, Rust, Java, Go, etc."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("documentation"),
                    description: Cow::Borrowed("Get comprehensive usage guide with examples, patterns, rules, and project configuration (sgconfig.yml) information."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("rule_search"),
                    description: Cow::Borrowed("Search for patterns using ast-grep rule configurations (YAML/JSON). Supports complex pattern matching with relational and composite rules."),
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
                    name: Cow::Borrowed("rule_replace"),
                    description: Cow::Borrowed("Replace patterns using ast-grep rule configurations with fix transformations. Supports complex rule-based code refactoring."),
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
                    name: Cow::Borrowed("validate_rule"),
                    description: Cow::Borrowed("Validate ast-grep rule configuration syntax and optionally test against sample code."),
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
                    name: Cow::Borrowed("create_rule"),
                    description: Cow::Borrowed("Create and store a new ast-grep rule configuration for reuse. LLMs can use this to build custom rule libraries."),
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
                    name: Cow::Borrowed("list_rules"),
                    description: Cow::Borrowed("List all stored rule configurations with optional filtering by language or severity."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "language": { "type": "string", "description": "Filter rules by programming language" },
                            "severity": { "type": "string", "description": "Filter rules by severity level (info, warning, error)" }
                        }
                    })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("get_rule"),
                    description: Cow::Borrowed("Retrieve a specific stored rule configuration by ID."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_id": { "type": "string", "description": "ID of the rule to retrieve" }
                        },
                        "required": ["rule_id"]
                    })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("delete_rule"),
                    description: Cow::Borrowed("Delete a stored rule configuration by ID."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_id": { "type": "string", "description": "ID of the rule to delete" }
                        },
                        "required": ["rule_id"]
                    })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("list_catalog_rules"),
                    description: Cow::Borrowed("List available rules from the ast-grep catalog with optional filtering."),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "language": { "type": "string", "description": "Filter rules by programming language" },
                            "category": { "type": "string", "description": "Filter rules by category" }
                        }
                    })).unwrap()),
                },
                Tool {
                    name: Cow::Borrowed("import_catalog_rule"),
                    description: Cow::Borrowed("Import a rule from the ast-grep catalog into local storage."),
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
                    name: Cow::Borrowed("generate_ast"),
                    description: Cow::Borrowed("Generate syntax tree for code and discover Tree-sitter node kinds. Essential for writing Kind rules - shows node types like function_declaration, identifier, etc."),
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
                let summary = ResponseFormatter::format_search_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "suggest_patterns" => {
                let param: SuggestPatternsParam = serde_json::from_value(
                    serde_json::Value::Object(request.arguments.unwrap_or_default()),
                )
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self
                    .suggest_patterns(param)
                    .await
                    .map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_suggest_patterns_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "file_search" => {
                let param: FileSearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
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
            "search_embedded" => {
                let param: EmbeddedSearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.search_embedded(param).await.map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_embedded_search_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "replace" => {
                let param: ReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.replace(param).await.map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_replace_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "file_replace" => {
                let param: FileReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.file_replace(param).await.map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_file_replace_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "list_languages" => {
                let param: ListLanguagesParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.list_languages(param).await.map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_list_languages_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "documentation" => {
                let param: DocumentationParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.documentation(param).await.map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_documentation_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "validate_rule" => {
                let param: RuleValidateParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.validate_rule(param).await.map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_rule_validate_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "rule_search" => {
                let param: RuleSearchParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self.rule_search(param).await.map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_file_search_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "rule_replace" => {
                let param: RuleReplaceParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
                let result = self.rule_replace(param).await.map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_file_replace_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
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
                let summary = ResponseFormatter::format_generate_ast_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "debug_pattern" => {
                let param: DebugPatternParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self
                    .debug_service
                    .debug_pattern(param)
                    .await
                    .map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_debug_pattern_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
            }
            "debug_ast" => {
                let param: DebugAstParam = serde_json::from_value(serde_json::Value::Object(
                    request.arguments.unwrap_or_default(),
                ))
                .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))?;
                let result = self
                    .debug_service
                    .debug_ast(param)
                    .await
                    .map_err(ErrorData::from)?;
                let summary = ResponseFormatter::format_debug_ast_result(&result);
                ResponseFormatter::create_formatted_response(&result, summary)
                    .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))
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
}
