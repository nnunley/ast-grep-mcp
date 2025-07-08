use ast_grep_mcp::ast_grep_service::{AstGrepService, RuleSearchParam, RuleValidateParam};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_rule_validation() {
    let service = AstGrepService::new();

    // Test valid YAML rule
    let yaml_rule = r#"
id: test-rule
language: javascript
message: "Found console.log"
severity: warning
rule:
  pattern: "console.log($ARG)"
"#;

    let param = RuleValidateParam {
        rule_config: yaml_rule.to_string(),
        test_code: Some("console.log('hello');".to_string()),
    };

    let result = service.validate_rule(param).await.unwrap();
    assert!(result.is_valid);
    assert!(result.errors.is_empty());
    assert!(result.test_matches.is_some());
    assert_eq!(result.test_matches.unwrap().len(), 1);
}

#[tokio::test]
async fn test_rule_validation_json() {
    let service = AstGrepService::new();

    // Test valid JSON rule
    let json_rule = r#"{
        "id": "test-rule",
        "language": "javascript", 
        "message": "Found console.log",
        "severity": "warning",
        "rule": {
            "pattern": "console.log($ARG)"
        }
    }"#;

    let param = RuleValidateParam {
        rule_config: json_rule.to_string(),
        test_code: Some("console.log('hello');".to_string()),
    };

    let result = service.validate_rule(param).await.unwrap();
    assert!(result.is_valid);
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn test_rule_validation_invalid() {
    let service = AstGrepService::new();

    // Test invalid rule (missing language)
    let invalid_rule = r#"
id: test-rule
rule:
  pattern: "console.log($ARG)"
"#;

    let param = RuleValidateParam {
        rule_config: invalid_rule.to_string(),
        test_code: None,
    };

    let result = service.validate_rule(param).await.unwrap();
    assert!(!result.is_valid);
    assert!(!result.errors.is_empty());
}

#[tokio::test]
async fn test_rule_search_basic() {
    use ast_grep_mcp::config::ServiceConfig;

    let temp_dir = TempDir::new().unwrap();

    // Create service with custom config pointing to temp directory
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create a test JavaScript file
    let test_file = temp_dir.path().join("test.js");
    fs::write(&test_file, "console.log('hello');\nconsole.error('error');").unwrap();

    let yaml_rule = r#"
id: find-console-log
language: javascript
message: "Found console.log usage"
severity: warning
rule:
  pattern: "console.log($ARG)"
"#;

    let param = RuleSearchParam {
        rule_config: yaml_rule.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: None,
        max_file_size: None,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();
    assert_eq!(result.rule_id, "find-console-log");
    assert_eq!(result.matches.len(), 1); // One file with matches
    assert_eq!(result.matches[0].matches.len(), 1); // One match in the file
    assert_eq!(
        result.matches[0].message,
        Some("Found console.log usage".to_string())
    );
    assert_eq!(result.matches[0].severity, Some("warning".to_string()));
}

#[tokio::test]
async fn test_rule_search_composite_all() {
    use ast_grep_mcp::config::ServiceConfig;

    let temp_dir = TempDir::new().unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create a test JavaScript file
    let test_file = temp_dir.path().join("test.js");
    fs::write(&test_file, "console.log('hello');\nconsole.error('error');").unwrap();

    // Test composite "all" rule (currently limited support)
    let yaml_rule = r#"
id: find-console-calls
language: javascript
message: "Found console calls"
severity: info
rule:
  all:
    - pattern: "console.log($ARG)"
    - pattern: "console.$METHOD($ARG)"
"#;

    let param = RuleSearchParam {
        rule_config: yaml_rule.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: None,
        max_file_size: None,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();
    assert_eq!(result.rule_id, "find-console-calls");
    // Since we currently use first pattern only, we expect console.log matches
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 1);
}

#[tokio::test]
async fn test_rule_replace_basic() {
    use ast_grep_mcp::ast_grep_service::{RuleReplaceParam, ServiceConfig};

    let temp_dir = TempDir::new().unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create a test JavaScript file
    let test_file = temp_dir.path().join("test.js");
    fs::write(&test_file, "console.log('hello');\nconsole.log('world');").unwrap();

    let yaml_rule = r#"
id: replace-console-log
language: javascript
message: "Replace console.log with console.debug"
severity: info
rule:
  pattern: "console.log($ARG)"
fix: "console.debug($ARG)"
"#;

    let param = RuleReplaceParam {
        rule_config: yaml_rule.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: None,
        max_file_size: None,
        dry_run: Some(true), // Dry run for testing
        summary_only: Some(false),
        cursor: None,
    };

    let result = service.rule_replace(param).await.unwrap();
    assert_eq!(result.rule_id, "replace-console-log");
    assert_eq!(result.total_changes, 2); // Two replacements
    assert!(result.dry_run); // Should be dry run
    assert_eq!(result.file_results.len(), 1); // One file processed
}

#[tokio::test]
async fn test_rule_management_lifecycle() {
    use ast_grep_mcp::ast_grep_service::{
        CreateRuleParam, DeleteRuleParam, GetRuleParam, ListRulesParam, ServiceConfig,
    };

    let temp_dir = TempDir::new().unwrap();

    let config = ServiceConfig {
        rules_directory: temp_dir.path().join("custom-rules"),
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let yaml_rule = r#"
id: test-rule-management
language: javascript
message: "Test rule for management functionality"
severity: warning
rule:
  pattern: "console.log($ARG)"
fix: "console.debug($ARG)"
"#;

    // Test creating a rule
    let create_param = CreateRuleParam {
        rule_config: yaml_rule.to_string(),
        overwrite: None,
    };

    let create_result = service.create_rule(create_param).await.unwrap();
    assert_eq!(create_result.rule_id, "test-rule-management");
    assert!(create_result.created);
    assert!(
        create_result
            .file_path
            .contains("test-rule-management.yaml")
    );

    // Test listing rules
    let list_param = ListRulesParam {
        language: None,
        severity: None,
    };

    let list_result = service.list_rules(list_param).await.unwrap();
    assert_eq!(list_result.rules.len(), 1);
    assert_eq!(list_result.rules[0].id, "test-rule-management");
    assert_eq!(list_result.rules[0].language, "javascript");
    assert!(list_result.rules[0].has_fix);

    // Test getting a specific rule
    let get_param = GetRuleParam {
        rule_id: "test-rule-management".to_string(),
    };

    let get_result = service.get_rule(get_param).await.unwrap();
    assert!(get_result.rule_config.contains("test-rule-management"));
    assert!(get_result.rule_config.contains("console.log"));

    // Test deleting a rule
    let delete_param = DeleteRuleParam {
        rule_id: "test-rule-management".to_string(),
    };

    let delete_result = service.delete_rule(delete_param).await.unwrap();
    assert_eq!(delete_result.rule_id, "test-rule-management");
    assert!(delete_result.deleted);

    // Verify rule is gone
    let list_result_after = service
        .list_rules(ListRulesParam {
            language: None,
            severity: None,
        })
        .await
        .unwrap();
    assert_eq!(list_result_after.rules.len(), 0);
}

#[tokio::test]
async fn test_rule_creation_with_overwrite() {
    use ast_grep_mcp::ast_grep_service::{CreateRuleParam, ServiceConfig};

    let temp_dir = TempDir::new().unwrap();

    let config = ServiceConfig {
        rules_directory: temp_dir.path().join("custom-rules"),
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let rule_v1 = r#"
id: test-overwrite
language: javascript
message: "Version 1"
rule:
  pattern: "console.log($ARG)"
"#;

    let rule_v2 = r#"
id: test-overwrite
language: javascript
message: "Version 2"
rule:
  pattern: "console.log($ARG)"
fix: "console.debug($ARG)"
"#;

    // Create initial rule
    let create_result1 = service
        .create_rule(CreateRuleParam {
            rule_config: rule_v1.to_string(),
            overwrite: None,
        })
        .await
        .unwrap();
    assert!(create_result1.created);

    // Try to create again without overwrite - should fail
    let create_result2 = service
        .create_rule(CreateRuleParam {
            rule_config: rule_v2.to_string(),
            overwrite: Some(false),
        })
        .await;
    assert!(create_result2.is_err());

    // Create with overwrite - should succeed
    let create_result3 = service
        .create_rule(CreateRuleParam {
            rule_config: rule_v2.to_string(),
            overwrite: Some(true),
        })
        .await
        .unwrap();
    assert!(!create_result3.created); // Should be false since it was updated

    // Verify the rule was updated
    let get_result = service
        .get_rule(ast_grep_mcp::ast_grep_service::GetRuleParam {
            rule_id: "test-overwrite".to_string(),
        })
        .await
        .unwrap();
    assert!(get_result.rule_config.contains("Version 2"));
    assert!(get_result.rule_config.contains("fix:"));
}

#[tokio::test]
async fn test_composite_rule_all() {
    use ast_grep_mcp::ast_grep_service::{RuleSearchParam, ServiceConfig};

    let temp_dir = TempDir::new().unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create a test JavaScript file
    let test_file = temp_dir.path().join("test.js");
    fs::write(
        &test_file,
        "console.log('hello');\nfunction test() { console.error('error'); }",
    )
    .unwrap();

    // Test "all" composite rule - should find nodes that match ALL patterns
    let yaml_rule = r#"
id: test-composite-all
language: javascript
message: "Found console method in function"
severity: info
rule:
  all:
    - pattern: "console.$METHOD($ARG)"
    - regex: "error"
"#;

    let param = RuleSearchParam {
        rule_config: yaml_rule.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: None,
        max_file_size: None,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();
    assert_eq!(result.rule_id, "test-composite-all");
    // Should find matches that satisfy both conditions
    assert!(!result.matches.is_empty());
}

#[tokio::test]
async fn test_composite_rule_any() {
    use ast_grep_mcp::ast_grep_service::{RuleSearchParam, ServiceConfig};

    let temp_dir = TempDir::new().unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create a test JavaScript file
    let test_file = temp_dir.path().join("test.js");
    fs::write(
        &test_file,
        "console.log('hello');\nconsole.error('error');\nconsole.warn('warning');",
    )
    .unwrap();

    // Test "any" composite rule - should find nodes that match ANY pattern
    let yaml_rule = r#"
id: test-composite-any
language: javascript
message: "Found console usage"
severity: info
rule:
  any:
    - pattern: "console.log($ARG)"
    - pattern: "console.error($ARG)"
    - pattern: "console.warn($ARG)"
"#;

    let param = RuleSearchParam {
        rule_config: yaml_rule.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: None,
        max_file_size: None,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();
    assert_eq!(result.rule_id, "test-composite-any");
    // Should find all three console method calls
    assert!(!result.matches.is_empty());

    // Check that we found multiple matches
    let total_matches: usize = result.matches.iter().map(|m| m.matches.len()).sum();
    assert_eq!(total_matches, 3); // Three console calls
}

#[tokio::test]
async fn test_composite_rule_not() {
    use ast_grep_mcp::ast_grep_service::{RuleSearchParam, ServiceConfig};

    let temp_dir = TempDir::new().unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create a test JavaScript file
    let test_file = temp_dir.path().join("test.js");
    fs::write(
        &test_file,
        "console.log('hello');\nfunction test() { return 42; }",
    )
    .unwrap();

    // Test "not" composite rule - should find nodes that DON'T match the pattern
    let yaml_rule = r#"
id: test-composite-not
language: javascript
message: "Found non-console code"
severity: info
rule:
  not:
    pattern: "console.$METHOD($ARG)"
"#;

    let param = RuleSearchParam {
        rule_config: yaml_rule.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: None,
        max_file_size: None,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();
    assert_eq!(result.rule_id, "test-composite-not");
    // Should find matches that are NOT console calls
    assert!(!result.matches.is_empty());
}

#[tokio::test]
async fn test_rule_with_regex() {
    use ast_grep_mcp::ast_grep_service::{RuleSearchParam, ServiceConfig};

    let temp_dir = TempDir::new().unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create a test JavaScript file
    let test_file = temp_dir.path().join("test.js");
    fs::write(
        &test_file,
        "const ERROR_CODE = 500;\nconst SUCCESS_CODE = 200;",
    )
    .unwrap();

    // Test regex rule
    let yaml_rule = r#"
id: test-regex-rule
language: javascript
message: "Found error-related code"
severity: warning
rule:
  regex: "ERROR"
"#;

    let param = RuleSearchParam {
        rule_config: yaml_rule.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: None,
        max_file_size: None,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();
    assert_eq!(result.rule_id, "test-regex-rule");
    // Should find the ERROR text
    assert!(!result.matches.is_empty());
    assert!(
        result.matches[0]
            .matches
            .iter()
            .any(|m| m.text.contains("ERROR"))
    );
}
