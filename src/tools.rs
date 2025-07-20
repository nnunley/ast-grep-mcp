use crate::rules::{RuleValidateParam, validate_rule};
use crate::types::*;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Content, ErrorData, ListToolsResult, Tool,
};
use serde_json;
use std::borrow::Cow;
use std::sync::Arc;

pub struct ToolService;

impl ToolService {
    pub fn list_tools() -> ListToolsResult {
        ListToolsResult {
            tools: vec![
                Tool {
                    name: "search".into(),
                    description: Some("Search for AST patterns in code strings. Use $VAR to capture single nodes, $$$ for multiple nodes (lists). Example patterns: 'console.log($MSG)', 'function $NAME($PARAMS) { $$$ }'. Returns matches with precise line/column positions and captured variables.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "code": { "type": "string", "description": "Source code to search in" },
                            "pattern": { "type": "string", "description": "AST pattern to search for. Use $VAR for single captures, $$$ for multiple captures" },
                            "language": { "type": "string", "description": "Programming language (javascript, typescript, python, rust, java, go, cpp, etc.)" },
                            "strictness": { "type": "string", "enum": ["cst", "smart", "ast", "relaxed", "signature"], "description": "Match strictness level" },
                            "selector": { "type": "string", "description": "CSS-like selector for matching specific node types" },
                            "context": { "type": "string", "description": "Context pattern to match surrounding code" },
                            "context_before": { "type": "integer", "minimum": 0, "description": "Number of lines to show before each match" },
                            "context_after": { "type": "integer", "minimum": 0, "description": "Number of lines to show after each match" },
                            "context_lines": { "type": "integer", "minimum": 0, "description": "Number of lines to show before and after each match (equivalent to grep -C)" }
                        },
                        "required": ["code", "pattern", "language"]
                    })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "file_search".into(),
                    description: Some("Search files for AST patterns using glob patterns. Use path_pattern like '**/*.js' or 'src/**/*.{ts,tsx}'. Supports pagination with cursor for large codebases. Returns matches grouped by file with context lines.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string", "description": "Glob pattern for files to search (e.g., '**/*.js', 'src/**/*.{ts,tsx}')" },
                            "pattern": { "type": "string", "description": "AST pattern to search for. Use $VAR for single captures, $$$ for multiple captures" },
                            "language": { "type": "string", "description": "Programming language of target files" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "default": 20, "description": "Maximum number of matches to return" },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824, "description": "Maximum file size to search in bytes" },
                            "cursor": {
                                "type": "object",
                                "properties": {
                                    "last_file_path": { "type": "string" },
                                    "is_complete": { "type": "boolean" }
                                },
                                "required": ["last_file_path", "is_complete"]
                            },
                            "strictness": { "type": "string", "enum": ["cst", "smart", "ast", "relaxed", "signature"], "description": "Match strictness level" },
                            "selector": { "type": "string", "description": "CSS-like selector for matching specific node types" },
                            "context": { "type": "string", "description": "Context pattern to match surrounding code" },
                            "context_before": { "type": "integer", "minimum": 0, "description": "Number of lines to show before each match" },
                            "context_after": { "type": "integer", "minimum": 0, "description": "Number of lines to show after each match" },
                            "context_lines": { "type": "integer", "minimum": 0, "description": "Number of lines to show before and after each match (equivalent to grep -C)" }
                        },
                        "required": ["path_pattern", "pattern", "language"]
                    })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "replace".into(),
                    description: Some("Replace AST patterns in code strings. Use $VAR in both pattern and replacement to preserve captured nodes. Example: pattern 'console.log($MSG)', replacement 'console.warn($MSG)'. Returns the modified code with changes applied.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "code": { "type": "string", "description": "Source code to modify" },
                            "pattern": { "type": "string", "description": "AST pattern to find and replace" },
                            "replacement": { "type": "string", "description": "Replacement pattern with captured variables (e.g., use $VAR from pattern)" },
                            "language": { "type": "string", "description": "Programming language of the code" }
                        },
                        "required": ["code", "pattern", "replacement", "language"]
                    })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "file_replace".into(),
                    description: Some("Replace AST patterns in multiple files using glob patterns. Use summary_only=true for bulk refactoring (returns counts instead of full diffs). Supports dry_run for preview. Essential for large-scale codebase modifications.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path_pattern": { "type": "string", "description": "Glob pattern for files to modify (e.g., '**/*.js', 'src/**/*.{ts,tsx}')" },
                            "pattern": { "type": "string", "description": "AST pattern to find and replace" },
                            "replacement": { "type": "string", "description": "Replacement pattern with captured variables" },
                            "language": { "type": "string", "description": "Programming language of target files" },
                            "max_results": { "type": "integer", "minimum": 1, "maximum": 10000, "description": "Maximum number of matches to process" },
                            "max_file_size": { "type": "integer", "minimum": 1024, "maximum": 1073741824, "description": "Maximum file size to process in bytes" },
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
                    annotations: None,
                },
                Tool {
                    name: "list_languages".into(),
                    description: Some("Get all supported programming languages for AST pattern matching. Returns 20+ languages including javascript, typescript, python, rust, java, go, cpp, csharp, etc. Use these exact language names in other tools.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({ "type": "object", "properties": {} })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "rule_search".into(),
                    description: Some("Search using ast-grep YAML rule configurations. Rules support complex patterns with conditions, constraints, and relational matching. More powerful than simple patterns - use for advanced searches requiring logical conditions or multiple pattern combinations.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML rule configuration with id, language, rule (pattern/kind/regex), and optional constraints" },
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
                    annotations: None,
                },
                Tool {
                    name: "rule_replace".into(),
                    description: Some("Replace using ast-grep YAML rule configurations with 'fix' transformations. Rules can include conditions and complex replacement logic. Essential for sophisticated refactoring beyond simple find-replace patterns.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML rule configuration with id, language, rule, and fix field for replacements" },
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
                    annotations: None,
                },
                Tool {
                    name: "validate_rule".into(),
                    description: Some("Validate ast-grep YAML rule syntax and test against sample code. Use this to verify rule configurations before using them in rule_search or rule_replace. Returns validation errors or successful match results.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "YAML rule configuration to validate (must include id, language, and rule fields)" },
                            "test_code": { "type": "string", "description": "Optional code sample to test the rule against" }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "create_rule".into(),
                    description: Some("Create and store a new ast-grep rule configuration for reuse. Build a library of custom rules for common patterns. Stored rules can be retrieved with get_rule and deleted with delete_rule.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_config": { "type": "string", "description": "Complete YAML rule configuration with id, language, rule, and optional fix/constraints" },
                            "overwrite": { "type": "boolean", "default": false, "description": "Whether to overwrite existing rule with same ID" }
                        },
                        "required": ["rule_config"]
                    })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "list_rules".into(),
                    description: Some("List all stored rule configurations with optional filtering. Shows rule IDs, languages, and descriptions. Use to discover available rules before using get_rule to retrieve specific configurations.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "language": { "type": "string", "description": "Filter rules by programming language" },
                            "severity": { "type": "string", "description": "Filter rules by severity level (info, warning, error)" }
                        }
                    })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "get_rule".into(),
                    description: Some("Retrieve a specific stored rule configuration by its ID. Returns the complete YAML rule configuration that can be used directly with rule_search or rule_replace tools.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_id": { "type": "string", "description": "ID of the rule to retrieve" }
                        },
                        "required": ["rule_id"]
                    })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "delete_rule".into(),
                    description: Some("Delete a stored rule configuration by its ID. Permanently removes the rule from storage. Use list_rules to see available rule IDs before deletion.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "rule_id": { "type": "string", "description": "ID of the rule to delete" }
                        },
                        "required": ["rule_id"]
                    })).unwrap()),
                    annotations: None,
                },
                Tool {
                    name: "generate_ast".into(),
                    description: Some("Generate Abstract Syntax Tree for code and discover Tree-sitter node kinds. Essential for writing Kind-based rules - shows exact node types like function_declaration, identifier, call_expression. Use when you need to know the precise AST structure for advanced pattern matching.".into()),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "code": { "type": "string", "description": "Source code to parse and generate AST for" },
                            "language": { "type": "string", "description": "Programming language for correct AST generation" }
                        },
                        "required": ["code", "language"]
                    })).unwrap()),
                    annotations: None,
                },
            ],
            ..Default::default()
        }
    }

    pub fn parse_param<T>(request: &CallToolRequestParam) -> Result<T, ErrorData>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_value(serde_json::Value::Object(
            request.arguments.clone().unwrap_or_default(),
        ))
        .map_err(|e| ErrorData::invalid_params(Cow::Owned(e.to_string()), None))
    }

    pub fn create_success_result<T>(result: &T) -> Result<CallToolResult, ErrorData>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(result)
            .map_err(|e| ErrorData::internal_error(Cow::Owned(e.to_string()), None))?;
        Ok(CallToolResult::success(vec![Content::json(json_value)?]))
    }

    pub async fn handle_validate_rule(
        param: RuleValidateParam,
    ) -> Result<CallToolResult, ErrorData> {
        let result = validate_rule(param).await.map_err(ErrorData::from)?;
        Self::create_success_result(&result)
    }

    pub fn list_languages() -> ListLanguagesResult {
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

        ListLanguagesResult { languages }
    }
}
