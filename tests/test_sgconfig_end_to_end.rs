//! End-to-end integration tests for sgconfig.yml support

use ast_grep_mcp::{
    CreateRuleParam, FileSearchParam, GetRuleParam, ListRulesParam,
    ast_grep_service::AstGrepService, config::ServiceConfig,
};
use std::fs;
use tempfile::TempDir;

/// Create a test project structure with sgconfig.yml and multiple rule directories
fn setup_test_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create project structure
    let src_dir = temp_dir.path().join("src");
    let lib_dir = src_dir.join("lib");
    let tests_dir = temp_dir.path().join("tests");
    let rules_dir = temp_dir.path().join("rules");
    let team_rules_dir = temp_dir.path().join("team-rules");
    let security_rules_dir = temp_dir.path().join("security-rules");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&lib_dir).unwrap();
    fs::create_dir_all(&tests_dir).unwrap();
    fs::create_dir_all(&rules_dir).unwrap();
    fs::create_dir_all(&team_rules_dir).unwrap();
    fs::create_dir_all(&security_rules_dir).unwrap();

    // Create sgconfig.yml
    let sgconfig_content = r#"
ruleDirs:
  - ./rules
  - ./team-rules
  - ./security-rules
utilDirs:
  - ./utils
"#;
    fs::write(temp_dir.path().join("sgconfig.yml"), sgconfig_content).unwrap();

    // Create some JavaScript files
    let js_code1 = r#"
function processData(data) {
    console.log("Processing:", data);
    if (data.length === 0) {
        console.error("No data to process");
        return;
    }
    console.log("Processed successfully");
}
"#;
    fs::write(src_dir.join("main.js"), js_code1).unwrap();

    let js_code2 = r#"
export function debugLog(message) {
    console.debug(message);
}

export function errorLog(error) {
    console.error("Error:", error);
}
"#;
    fs::write(lib_dir.join("logger.js"), js_code2).unwrap();

    // Create test file
    let test_code = r#"
import { debugLog } from '../src/lib/logger';

test('logging test', () => {
    console.log("Running test");
    debugLog("Debug message");
});
"#;
    fs::write(tests_dir.join("logger.test.js"), test_code).unwrap();

    // Create rules in different directories

    // Rule in main rules directory
    let console_rule = r#"
id: no-console-log
message: Avoid using console.log in production code
severity: warning
language: javascript
rule:
  pattern: console.log($$$)
fix: |
  // console.log($$$)
"#;
    fs::write(rules_dir.join("no-console-log.yaml"), console_rule).unwrap();

    // Rule in team-rules directory
    let error_handling_rule = r#"
id: proper-error-logging
message: Use structured error logging
severity: error
language: javascript
rule:
  pattern: console.error($MSG)
  not:
    pattern: console.error("Error:", $$$)
fix: |
  console.error("Error:", $MSG)
"#;
    fs::write(
        team_rules_dir.join("proper-error-logging.yaml"),
        error_handling_rule,
    )
    .unwrap();

    // Rule in security-rules directory
    let debug_rule = r#"
id: no-debug-in-production
message: Remove debug statements before deployment
severity: error
language: javascript
rule:
  kind: call_expression
  has:
    kind: member_expression
    regex: "console\\.(debug|trace)"
"#;
    fs::write(
        security_rules_dir.join("no-debug-in-production.yaml"),
        debug_rule,
    )
    .unwrap();

    temp_dir
}

#[tokio::test]
async fn test_sgconfig_rule_discovery() {
    let project_dir = setup_test_project();

    // Create service with sgconfig.yml discovery
    let config = ServiceConfig {
        root_directories: vec![project_dir.path().to_path_buf()],
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // List all rules - should find rules from all directories
    let list_param = ListRulesParam {
        language: Some("javascript".to_string()),
        severity: None,
    };

    let rules = service.list_rules(list_param).await.unwrap();
    assert_eq!(rules.rules.len(), 3);

    // Verify we have rules from all directories
    let rule_ids: Vec<&str> = rules.rules.iter().map(|r| r.id.as_str()).collect();
    assert!(rule_ids.contains(&"no-console-log"));
    assert!(rule_ids.contains(&"proper-error-logging"));
    assert!(rule_ids.contains(&"no-debug-in-production"));
}

#[tokio::test]
async fn test_sgconfig_file_search_with_rules() {
    let project_dir = setup_test_project();

    // Create service with sgconfig.yml discovery
    let config = ServiceConfig {
        root_directories: vec![project_dir.path().to_path_buf()],
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // Search for console.log usage
    let search_param = FileSearchParam {
        pattern: "console.log($$$)".to_string(),
        language: "javascript".to_string(),
        path_pattern: "**/*.js".to_string(),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
        strictness: None,
        selector: None,
        context: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let result = service.file_search(search_param).await.unwrap();

    // Should find matches in main.js and logger.test.js
    assert_eq!(result.matches.len(), 2);

    // Verify matches
    let file_paths: Vec<&str> = result
        .matches
        .iter()
        .map(|m| m.file_path.as_str())
        .collect();

    assert!(file_paths.iter().any(|p| p.ends_with("src/main.js")));
    assert!(
        file_paths
            .iter()
            .any(|p| p.ends_with("tests/logger.test.js"))
    );
}

#[tokio::test]
async fn test_sgconfig_get_rule_from_different_dirs() {
    let project_dir = setup_test_project();

    // Create service with sgconfig.yml discovery
    let config = ServiceConfig {
        root_directories: vec![project_dir.path().to_path_buf()],
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // Get rule from main rules directory
    let get_param1 = GetRuleParam {
        rule_id: "no-console-log".to_string(),
    };
    let rule1 = service.get_rule(get_param1).await.unwrap();
    assert_eq!(rule1.rule_config.id, "no-console-log");
    assert!(rule1.file_path.contains("rules"));

    // Get rule from team-rules directory
    let get_param2 = GetRuleParam {
        rule_id: "proper-error-logging".to_string(),
    };
    let rule2 = service.get_rule(get_param2).await.unwrap();
    assert_eq!(rule2.rule_config.id, "proper-error-logging");
    assert!(rule2.file_path.contains("team-rules"));

    // Get rule from security-rules directory
    let get_param3 = GetRuleParam {
        rule_id: "no-debug-in-production".to_string(),
    };
    let rule3 = service.get_rule(get_param3).await.unwrap();
    assert_eq!(rule3.rule_config.id, "no-debug-in-production");
    assert!(rule3.file_path.contains("security-rules"));
}

#[tokio::test]
async fn test_sgconfig_create_rule_in_primary_dir() {
    let project_dir = setup_test_project();

    // Create service with sgconfig.yml discovery
    let config = ServiceConfig {
        root_directories: vec![project_dir.path().to_path_buf()],
        rules_directory: project_dir.path().join(".ast-grep-rules"), // Custom primary directory
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // Create a new rule
    let new_rule = r#"
id: new-custom-rule
message: Custom rule for testing
language: javascript
rule:
  pattern: test($ARG)
"#;

    let create_param = CreateRuleParam {
        rule_config: new_rule.to_string(),
        overwrite: false,
    };

    let result = service.create_rule(create_param).await.unwrap();
    assert!(result.created);
    assert_eq!(result.rule_id, "new-custom-rule");

    // Verify it was created in the primary directory
    assert!(
        project_dir
            .path()
            .join(".ast-grep-rules/new-custom-rule.yaml")
            .exists()
    );

    // Verify we can retrieve it
    let get_param = GetRuleParam {
        rule_id: "new-custom-rule".to_string(),
    };
    let retrieved = service.get_rule(get_param).await.unwrap();
    assert_eq!(retrieved.rule_config.id, "new-custom-rule");
}

#[tokio::test]
async fn test_sgconfig_explicit_path() {
    let project_dir = setup_test_project();
    let config_dir = project_dir.path().join("config");
    fs::create_dir_all(&config_dir).unwrap();

    // Create a different sgconfig.yml in config directory
    let alt_sgconfig = r#"
ruleDirs:
  - ../alternative-rules
"#;
    let alt_config_path = config_dir.join("alt-sgconfig.yml");
    fs::write(&alt_config_path, alt_sgconfig).unwrap();

    // Create alternative rules directory
    let alt_rules_dir = project_dir.path().join("alternative-rules");
    fs::create_dir_all(&alt_rules_dir).unwrap();

    // Create a rule in alternative directory
    let alt_rule = r#"
id: alternative-rule
message: Rule from alternative config
language: javascript
rule:
  pattern: alternative($$$)
"#;
    fs::write(alt_rules_dir.join("alternative-rule.yaml"), alt_rule).unwrap();

    // Create service with explicit config path and custom rules directory
    let config = ServiceConfig {
        root_directories: vec![project_dir.path().to_path_buf()],
        rules_directory: project_dir.path().join("primary-rules"), // Avoid default rules
        ..Default::default()
    }
    .with_sg_config(Some(&alt_config_path));

    let service = AstGrepService::with_config(config);

    // Should find the alternative rule
    let list_param = ListRulesParam {
        language: None,
        severity: None,
    };

    let rules = service.list_rules(list_param).await.unwrap();

    // Should find rules from alternative directory but not from the default sgconfig.yml
    let rule_ids: Vec<&str> = rules.rules.iter().map(|r| r.id.as_str()).collect();
    assert!(rule_ids.contains(&"alternative-rule"));
    assert!(!rule_ids.contains(&"no-console-log")); // Should not find rules from default config
}

#[tokio::test]
async fn test_sgconfig_nested_directory_discovery() {
    let project_dir = setup_test_project();
    let deep_dir = project_dir.path().join("src/components/ui/buttons");
    fs::create_dir_all(&deep_dir).unwrap();

    // Create a JS file in the deep directory
    let button_code = r#"
export function Button({ onClick, children }) {
    console.log("Button clicked");
    return <button onClick={onClick}>{children}</button>;
}
"#;
    fs::write(deep_dir.join("Button.js"), button_code).unwrap();

    // Create service starting from deep directory - should discover sgconfig.yml at root
    let config = ServiceConfig {
        root_directories: vec![deep_dir.clone()],
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // Should have discovered rules from root sgconfig.yml
    let list_param = ListRulesParam {
        language: Some("javascript".to_string()),
        severity: None,
    };

    let rules = service.list_rules(list_param).await.unwrap();
    assert_eq!(rules.rules.len(), 3); // All three rules from different directories
}

#[tokio::test]
async fn test_sgconfig_severity_filtering() {
    let project_dir = setup_test_project();

    // Create service with sgconfig.yml discovery
    let config = ServiceConfig {
        root_directories: vec![project_dir.path().to_path_buf()],
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // Filter by warning severity
    let warning_param = ListRulesParam {
        language: Some("javascript".to_string()),
        severity: Some("warning".to_string()),
    };

    let warning_rules = service.list_rules(warning_param).await.unwrap();
    assert_eq!(warning_rules.rules.len(), 1);
    assert_eq!(warning_rules.rules[0].id, "no-console-log");

    // Filter by error severity
    let error_param = ListRulesParam {
        language: Some("javascript".to_string()),
        severity: Some("error".to_string()),
    };

    let error_rules = service.list_rules(error_param).await.unwrap();
    assert_eq!(error_rules.rules.len(), 2);
    let error_ids: Vec<&str> = error_rules.rules.iter().map(|r| r.id.as_str()).collect();
    assert!(error_ids.contains(&"proper-error-logging"));
    assert!(error_ids.contains(&"no-debug-in-production"));
}
