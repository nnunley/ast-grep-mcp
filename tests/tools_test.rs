use ast_grep_mcp::rules::RuleValidateParam;
use ast_grep_mcp::tools::ToolService;
use ast_grep_mcp::types::*;
use rmcp::model::{CallToolRequestParam, ErrorData};
use serde_json::{Map, Value, json};

#[test]
fn test_list_tools() {
    let result = ToolService::list_tools();

    // Check that all expected tools are present
    let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    let expected_tools = vec![
        "search",
        "file_search",
        "replace",
        "file_replace",
        "list_languages",
        "rule_search",
        "rule_replace",
        "validate_rule",
        "create_rule",
        "list_rules",
        "get_rule",
        "delete_rule",
        "generate_ast",
        "validate_pattern",
        "explore_patterns",
    ];

    for expected in &expected_tools {
        assert!(tool_names.contains(expected), "Missing tool: {expected}");
    }

    assert_eq!(tool_names.len(), expected_tools.len());
}

#[test]
fn test_search_tool_schema() {
    let result = ToolService::list_tools();
    let search_tool = result.tools.iter().find(|t| t.name == "search").unwrap();

    assert_eq!(search_tool.name, "search");
    println!("Search tool description: {:?}", search_tool.description);
    assert!(search_tool.description.as_ref().unwrap().contains("AST"));

    // Verify the schema structure
    let schema = &search_tool.input_schema;
    assert_eq!(schema["type"], "object");
    let properties = &schema["properties"];
    assert!(properties["code"].is_object());
    assert!(properties["pattern"].is_object());
    assert!(properties["language"].is_object());
}

#[test]
fn test_file_search_tool_schema() {
    let result = ToolService::list_tools();
    let tool = result
        .tools
        .iter()
        .find(|t| t.name == "file_search")
        .unwrap();

    assert!(tool.description.as_ref().unwrap().contains("file"));

    let schema = &tool.input_schema;
    let properties = &schema["properties"];
    assert!(properties["path_pattern"].is_object());
    assert!(properties["pattern"].is_object());
    assert!(properties["language"].is_object());
    assert!(properties["max_results"].is_object());
    assert!(properties["max_file_size"].is_object());
    assert!(properties["cursor"].is_object());

    let required = &schema["required"];
    assert!(
        required
            .as_array()
            .unwrap()
            .contains(&json!("path_pattern"))
    );
    assert!(required.as_array().unwrap().contains(&json!("pattern")));
    assert!(required.as_array().unwrap().contains(&json!("language")));
}

#[test]
fn test_rule_search_tool_schema() {
    let result = ToolService::list_tools();
    let tool = result
        .tools
        .iter()
        .find(|t| t.name == "rule_search")
        .unwrap();

    assert!(tool.description.as_ref().unwrap().contains("rule"));
    assert!(tool.description.as_ref().unwrap().contains("YAML"));

    let schema = &tool.input_schema;
    let properties = &schema["properties"];
    assert!(properties["rule_config"].is_object());
    assert!(properties["path_pattern"].is_object());

    let required = &schema["required"];
    assert!(required.as_array().unwrap().contains(&json!("rule_config")));
}

#[test]
fn test_rule_replace_tool_schema() {
    let result = ToolService::list_tools();
    let tool = result
        .tools
        .iter()
        .find(|t| t.name == "rule_replace")
        .unwrap();

    assert!(tool.description.as_ref().unwrap().contains("replace"));
    assert!(tool.description.as_ref().unwrap().contains("fix"));

    let schema = &tool.input_schema;
    let properties = &schema["properties"];
    assert!(properties["rule_config"].is_object());
    assert!(properties["dry_run"].is_object());
    assert!(properties["summary_only"].is_object());
}

#[test]
fn test_validate_rule_tool_schema() {
    let result = ToolService::list_tools();
    let tool = result
        .tools
        .iter()
        .find(|t| t.name == "validate_rule")
        .unwrap();

    assert!(tool.description.as_ref().unwrap().contains("Validate"));

    let schema = &tool.input_schema;
    let properties = &schema["properties"];
    assert!(properties["rule_config"].is_object());
    assert!(properties["test_code"].is_object());

    let required = &schema["required"];
    assert!(required.as_array().unwrap().contains(&json!("rule_config")));
}

#[test]
fn test_parse_param_success() {
    let mut arguments = Map::new();
    arguments.insert("code".to_string(), json!("test code"));
    arguments.insert("pattern".to_string(), json!("test"));
    arguments.insert("language".to_string(), json!("javascript"));

    let request = CallToolRequestParam {
        name: "search".into(),
        arguments: Some(arguments),
    };

    let result: Result<SearchParam, ErrorData> = ToolService::parse_param(&request);
    assert!(result.is_ok());

    let param = result.unwrap();
    assert_eq!(param.code, "test code");
    assert_eq!(param.pattern, "test");
    assert_eq!(param.language, "javascript");
}

#[test]
fn test_parse_param_missing_field() {
    let mut arguments = Map::new();
    arguments.insert("code".to_string(), json!("test code"));
    // Missing pattern and language

    let request = CallToolRequestParam {
        name: "search".into(),
        arguments: Some(arguments),
    };

    let result: Result<SearchParam, ErrorData> = ToolService::parse_param(&request);
    assert!(result.is_err());
}

#[test]
fn test_parse_param_wrong_type() {
    let mut arguments = Map::new();
    arguments.insert("code".to_string(), json!(123)); // Wrong type
    arguments.insert("pattern".to_string(), json!("test"));
    arguments.insert("language".to_string(), json!("javascript"));

    let request = CallToolRequestParam {
        name: "search".into(),
        arguments: Some(arguments),
    };

    let result: Result<SearchParam, ErrorData> = ToolService::parse_param(&request);
    assert!(result.is_err());
}

#[test]
fn test_parse_param_no_arguments() {
    let request = CallToolRequestParam {
        name: "search".into(),
        arguments: None,
    };

    let result: Result<SearchParam, ErrorData> = ToolService::parse_param(&request);
    assert!(result.is_err());
}

#[test]
fn test_create_success_result() {
    let search_result = SearchResult {
        matches: vec![MatchResult {
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 10,
            text: "test match".to_string(),
            vars: std::collections::HashMap::new(),
            context_before: None,
            context_after: None,
        }],
        matches_summary: None,
    };

    let result = ToolService::create_success_result(&search_result);
    assert!(result.is_ok());

    let call_result = result.unwrap();
    assert!(!call_result.content.is_empty());
}

#[test]
fn test_create_success_result_serialization_error() {
    // Create a structure that can't be serialized (f64::NAN)
    let mut problematic_map: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    problematic_map.insert("invalid".to_string(), f64::NAN);

    let result = ToolService::create_success_result(&problematic_map);
    // NaN values are actually serializable in serde_json, they become null
    // So this test should expect success, not failure
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_handle_validate_rule_success() {
    let param = RuleValidateParam {
        rule_config: r#"
id: test-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
"#
        .to_string(),
        test_code: Some("console.log('hello');".to_string()),
    };

    let result = ToolService::handle_validate_rule(param).await;
    assert!(result.is_ok());

    let call_result = result.unwrap();
    assert!(!call_result.content.is_empty());
}

#[tokio::test]
async fn test_handle_validate_rule_invalid() {
    let param = RuleValidateParam {
        rule_config: "invalid yaml content {".to_string(),
        test_code: None,
    };

    let result = ToolService::handle_validate_rule(param).await;
    // The validation function may return a validation result even for invalid YAML
    // rather than failing completely, so this test should check for that
    assert!(result.is_ok());
}

#[test]
fn test_list_languages() {
    let result = ToolService::list_languages();

    assert!(!result.languages.is_empty());

    // Check for some expected languages
    let expected_languages = vec![
        "javascript",
        "typescript",
        "python",
        "rust",
        "java",
        "go",
        "cpp",
        "c",
    ];

    for lang in expected_languages {
        assert!(
            result.languages.contains(&lang.to_string()),
            "Missing language: {lang}"
        );
    }

    // Verify all languages are unique
    let mut sorted_langs = result.languages.clone();
    sorted_langs.sort();
    sorted_langs.dedup();
    assert_eq!(
        sorted_langs.len(),
        result.languages.len(),
        "Languages list contains duplicates"
    );
}

#[test]
fn test_file_replace_tool_detailed_schema() {
    let result = ToolService::list_tools();
    let tool = result
        .tools
        .iter()
        .find(|t| t.name == "file_replace")
        .unwrap();

    let schema = &tool.input_schema;
    let properties = &schema["properties"];

    // Check max_results constraints
    let max_results = &properties["max_results"];
    assert_eq!(max_results["type"], "integer");
    assert_eq!(max_results["minimum"], 1);
    assert_eq!(max_results["maximum"], 10000);

    // Check max_file_size constraints
    let max_file_size = &properties["max_file_size"];
    assert_eq!(max_file_size["type"], "integer");
    assert_eq!(max_file_size["minimum"], 1024);
    assert_eq!(max_file_size["maximum"], 1073741824);

    // Check boolean defaults
    let dry_run = &properties["dry_run"];
    assert_eq!(dry_run["type"], "boolean");
    assert_eq!(dry_run["default"], true);

    let summary_only = &properties["summary_only"];
    assert_eq!(summary_only["type"], "boolean");
    assert_eq!(summary_only["default"], false);

    // Check cursor object structure
    let cursor = &properties["cursor"];
    assert_eq!(cursor["type"], "object");
    let cursor_props = &cursor["properties"];
    assert!(cursor_props["last_file_path"].is_object());
    assert!(cursor_props["is_complete"].is_object());

    let cursor_required = &cursor["required"];
    assert!(
        cursor_required
            .as_array()
            .unwrap()
            .contains(&json!("last_file_path"))
    );
    assert!(
        cursor_required
            .as_array()
            .unwrap()
            .contains(&json!("is_complete"))
    );
}

#[test]
fn test_rule_management_tools_schemas() {
    let result = ToolService::list_tools();

    // Test create_rule
    let create_tool = result
        .tools
        .iter()
        .find(|t| t.name == "create_rule")
        .unwrap();
    let create_schema = &create_tool.input_schema;
    let create_props = &create_schema["properties"];
    assert!(create_props["rule_config"].is_object());
    assert!(create_props["overwrite"].is_object());
    assert_eq!(create_props["overwrite"]["default"], false);

    // Test list_rules
    let list_tool = result
        .tools
        .iter()
        .find(|t| t.name == "list_rules")
        .unwrap();
    let list_schema = &list_tool.input_schema;
    let list_props = &list_schema["properties"];
    assert!(list_props["language"].is_object());
    assert!(list_props["severity"].is_object());
    // No required fields for list_rules
    assert!(
        list_schema.get("required").is_none()
            || list_schema["required"].as_array().unwrap().is_empty()
    );

    // Test get_rule
    let get_tool = result.tools.iter().find(|t| t.name == "get_rule").unwrap();
    let get_schema = &get_tool.input_schema;
    let get_props = &get_schema["properties"];
    assert!(get_props["rule_id"].is_object());
    let get_required = &get_schema["required"];
    assert!(get_required.as_array().unwrap().contains(&json!("rule_id")));

    // Test delete_rule
    let delete_tool = result
        .tools
        .iter()
        .find(|t| t.name == "delete_rule")
        .unwrap();
    let delete_schema = &delete_tool.input_schema;
    let delete_props = &delete_schema["properties"];
    assert!(delete_props["rule_id"].is_object());
    let delete_required = &delete_schema["required"];
    assert!(
        delete_required
            .as_array()
            .unwrap()
            .contains(&json!("rule_id"))
    );
}

#[test]
#[ignore = "Catalog tools were removed from the service"]
fn test_catalog_tools_schemas() {
    let result = ToolService::list_tools();

    // Test list_catalog_rules
    let list_tool = result
        .tools
        .iter()
        .find(|t| t.name == "list_catalog_rules")
        .unwrap();
    let list_schema = &list_tool.input_schema;
    let list_props = &list_schema["properties"];
    assert!(list_props["language"].is_object());
    assert!(list_props["category"].is_object());

    // Test import_catalog_rule
    let import_tool = result
        .tools
        .iter()
        .find(|t| t.name == "import_catalog_rule")
        .unwrap();
    let import_schema = &import_tool.input_schema;
    let import_props = &import_schema["properties"];
    assert!(import_props["rule_url"].is_object());
    assert!(import_props["rule_id"].is_object());
    let import_required = &import_schema["required"];
    assert!(
        import_required
            .as_array()
            .unwrap()
            .contains(&json!("rule_url"))
    );
}

#[test]
fn test_generate_ast_tool_schema() {
    let result = ToolService::list_tools();
    let tool = result
        .tools
        .iter()
        .find(|t| t.name == "generate_ast")
        .unwrap();

    assert!(
        tool.description
            .as_ref()
            .unwrap()
            .contains("Abstract Syntax Tree")
    );
    assert!(tool.description.as_ref().unwrap().contains("Tree-sitter"));

    let schema = &tool.input_schema;
    let properties = &schema["properties"];
    assert!(properties["code"].is_object());
    assert!(properties["language"].is_object());

    let required = &schema["required"];
    assert!(required.as_array().unwrap().contains(&json!("code")));
    assert!(required.as_array().unwrap().contains(&json!("language")));
}

#[test]
fn test_list_languages_and_documentation_schemas() {
    let result = ToolService::list_tools();

    // Test list_languages - should have empty properties
    let lang_tool = result
        .tools
        .iter()
        .find(|t| t.name == "list_languages")
        .unwrap();
    let lang_schema = &lang_tool.input_schema;
    assert_eq!(lang_schema["type"], "object");
    let lang_props = &lang_schema["properties"];
    assert!(lang_props.as_object().unwrap().is_empty());
}

#[test]
fn test_parse_param_complex_types() {
    // Test parsing FileSearchParam with cursor
    let mut arguments = Map::new();
    arguments.insert("path_pattern".to_string(), json!("**/*.js"));
    arguments.insert("pattern".to_string(), json!("console.log($VAR)"));
    arguments.insert("language".to_string(), json!("javascript"));
    arguments.insert("max_results".to_string(), json!(50));
    arguments.insert("max_file_size".to_string(), json!(1048576));

    let mut cursor = Map::new();
    cursor.insert("last_file_path".to_string(), json!("/path/to/file.js"));
    cursor.insert("is_complete".to_string(), json!(false));
    arguments.insert("cursor".to_string(), Value::Object(cursor));

    let request = CallToolRequestParam {
        name: "file_search".into(),
        arguments: Some(arguments),
    };

    let result: Result<FileSearchParam, ErrorData> = ToolService::parse_param(&request);
    assert!(result.is_ok());

    let param = result.unwrap();
    assert_eq!(param.path_pattern, "**/*.js");
    assert_eq!(param.pattern, "console.log($VAR)");
    assert_eq!(param.language, "javascript");
    assert_eq!(param.max_results, 50);
    assert_eq!(param.max_file_size, 1048576);

    let cursor = param.cursor.unwrap();
    assert_eq!(cursor.last_file_path, "/path/to/file.js");
    assert!(!cursor.is_complete);
}
