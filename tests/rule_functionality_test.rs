use ast_grep_mcp::ast_grep_service::{AstGrepService, RuleValidateParam, RuleSearchParam};
use tempfile::TempDir;
use std::fs;

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
    use ast_grep_mcp::ast_grep_service::ServiceConfig;
    
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
    assert_eq!(result.matches[0].message, Some("Found console.log usage".to_string()));
    assert_eq!(result.matches[0].severity, Some("warning".to_string()));
}

#[tokio::test]
async fn test_rule_search_composite_all() {
    use ast_grep_mcp::ast_grep_service::ServiceConfig;
    
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
    use ast_grep_mcp::ast_grep_service::{ServiceConfig, RuleReplaceParam};
    
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