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
                    description: "Search for patterns in code using ast-grep.".into(),
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "code": { "type": "string" },
                            "pattern": { "type": "string" },
                            "language": { "type": "string" }
                        }
                    })).unwrap()),
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
                    input_schema: Arc::new(serde_json::from_value(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "code": { "type": "string" },
                            "pattern": { "type": "string" },
                            "replacement": { "type": "string" },
                            "language": { "type": "string" }
                        }
                    })).unwrap()),
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

    pub fn get_documentation() -> DocumentationResult {
        let docs = r##"
# AST-Grep MCP Service Documentation

This service provides structural code search and transformation using ast-grep patterns and rule configurations.

## Key Concepts

**AST Patterns:** Use `$VAR` to capture single nodes, `$$$` to capture multiple statements
**Languages:** Supports 20+ programming languages including JavaScript, TypeScript, Python, Rust, Java, etc.
**Rules:** YAML/JSON configurations for complex pattern matching with logical operations

## Basic Tools

### search
Search for patterns in code strings
```json
{
  "code": "console.log('hello'); console.log('world');",
  "pattern": "console.log($MSG)",
  "language": "javascript"
}
```

### file_search
Search for patterns across files using glob patterns
```json
{
  "path_pattern": "src/**/*.js",
  "pattern": "function $NAME($ARGS) { $BODY }",
  "language": "javascript"
}
```

### replace
Replace patterns in code strings
```json
{
  "code": "var x = 1; var y = 2;",
  "pattern": "var $VAR = $VALUE;",
  "replacement": "let $VAR = $VALUE;",
  "language": "javascript"
}
```

### file_replace
Replace patterns across files
```json
{
  "path_pattern": "src/**/*.js",
  "pattern": "console.log($MSG)",
  "replacement": "logger.info($MSG)",
  "language": "javascript",
  "dry_run": true
}
```

## Rule-Based Tools

### rule_search
Search using YAML/JSON rule configurations
```yaml
id: no-console-log
language: javascript
rule:
  pattern: console.log($ARGS)
```

### rule_replace
Replace using rules with fix transformations
```yaml
id: use-let
language: javascript
rule:
  pattern: var $VAR = $VALUE;
fix: let $VAR = $VALUE;
```

### Composite Rules
Combine multiple conditions with logical operators:
- `all`: Match nodes that satisfy ALL conditions
- `any`: Match nodes that satisfy ANY condition
- `not`: Match nodes that DON'T satisfy the condition

```yaml
id: complex-rule
language: javascript
rule:
  all:
    - pattern: function $NAME($ARGS) { $BODY }
    - not:
        pattern: function $NAME() { $BODY }
```

## Rule Management

### create_rule
Store custom rules for reuse
```json
{
  "rule_config": "id: my-rule\nlanguage: javascript\nrule:\n  pattern: $PATTERN"
}
```

### list_rules, get_rule, delete_rule
Manage your stored rule library

## Catalog Integration

### list_catalog_rules
Browse rules from the ast-grep catalog
```json
{
  "language": "javascript",
  "category": "best-practices"
}
```

### import_catalog_rule
Import rules from the online catalog
```json
{
  "rule_url": "https://ast-grep.github.io/catalog/javascript/no-console-log"
}
```

## Pattern Examples

**Variable captures:** `$VAR`, `$NAME`, `$ARGS`
**Multi-statement:** `$$$STATEMENTS`
**Optional elements:** Use composite rules with `any`
**Complex matching:** Combine `pattern`, `kind`, `regex` in rules

## Performance Features

- **Pattern caching:** Automatically caches compiled patterns
- **Pagination:** Large result sets with cursor-based pagination
- **File filtering:** Size limits and glob pattern filtering
- **Summary mode:** For bulk operations, get counts instead of full diffs

"##;

        DocumentationResult {
            content: docs.to_string(),
        }
    }
}
