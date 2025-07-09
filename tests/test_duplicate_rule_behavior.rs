//! Test to verify duplicate rule ID behavior matches ast-grep CLI

use ast_grep_mcp::{
    GetRuleParam, ListRulesParam, ast_grep_service::AstGrepService, config::ServiceConfig,
};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_duplicate_rule_id_first_wins() {
    // Based on ast-grep CLI behavior, when duplicate rule IDs exist,
    // typically the first one encountered takes precedence

    let temp_dir = TempDir::new().unwrap();

    // Create multiple rule directories
    let rules_dir1 = temp_dir.path().join("rules1");
    let rules_dir2 = temp_dir.path().join("rules2");
    let rules_dir3 = temp_dir.path().join("rules3");

    fs::create_dir_all(&rules_dir1).unwrap();
    fs::create_dir_all(&rules_dir2).unwrap();
    fs::create_dir_all(&rules_dir3).unwrap();

    // Create sgconfig.yml with ordered directories
    let sgconfig_content = r#"
ruleDirs:
  - ./rules1
  - ./rules2
  - ./rules3
"#;
    fs::write(temp_dir.path().join("sgconfig.yml"), sgconfig_content).unwrap();

    // Create the same rule ID with different content in each directory
    let rule1_content = r#"
id: duplicate-check
message: Rule from directory 1
severity: error
language: javascript
rule:
  pattern: console.log("from dir1")
"#;

    let rule2_content = r#"
id: duplicate-check
message: Rule from directory 2
severity: warning
language: javascript
rule:
  pattern: console.log("from dir2")
"#;

    let rule3_content = r#"
id: duplicate-check
message: Rule from directory 3
severity: info
language: javascript
rule:
  pattern: console.log("from dir3")
"#;

    fs::write(rules_dir1.join("duplicate-check.yaml"), rule1_content).unwrap();
    fs::write(rules_dir2.join("duplicate-check.yaml"), rule2_content).unwrap();
    fs::write(rules_dir3.join("duplicate-check.yaml"), rule3_content).unwrap();

    // Create service with a custom rules directory to avoid picking up default .ast-grep-rules
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        rules_directory: temp_dir.path().join("primary-rules"), // Set a non-existent primary directory
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // List rules should only show one instance
    let list_param = ListRulesParam {
        language: None,
        severity: None,
    };

    let rules = service.list_rules(list_param).await.unwrap();

    // Should only have one rule with ID "duplicate-check"
    let duplicate_rules: Vec<_> = rules
        .rules
        .iter()
        .filter(|r| r.id == "duplicate-check")
        .collect();
    assert_eq!(duplicate_rules.len(), 1);

    // Get the rule - should get the first one (from rules1)
    let get_param = GetRuleParam {
        rule_id: "duplicate-check".to_string(),
    };
    let rule = service.get_rule(get_param).await.unwrap();

    // Verify it's the rule from the first directory
    assert_eq!(
        rule.rule_config.message,
        Some("Rule from directory 1".to_string())
    );
    assert_eq!(rule.rule_config.severity, Some("error".to_string()));
    assert!(rule.file_path.contains("rules1"));
}

#[tokio::test]
async fn test_unique_rules_across_directories() {
    // Test that unique rule IDs from different directories all work correctly

    let temp_dir = TempDir::new().unwrap();

    // Create multiple rule directories
    let rules_dir1 = temp_dir.path().join("security");
    let rules_dir2 = temp_dir.path().join("performance");
    let rules_dir3 = temp_dir.path().join("style");

    fs::create_dir_all(&rules_dir1).unwrap();
    fs::create_dir_all(&rules_dir2).unwrap();
    fs::create_dir_all(&rules_dir3).unwrap();

    // Create sgconfig.yml
    let sgconfig_content = r#"
ruleDirs:
  - ./security
  - ./performance
  - ./style
"#;
    fs::write(temp_dir.path().join("sgconfig.yml"), sgconfig_content).unwrap();

    // Create unique rules in each directory
    let security_rule = r#"
id: no-eval
message: Avoid using eval for security reasons
severity: error
language: javascript
rule:
  pattern: eval($$$)
"#;

    let performance_rule = r#"
id: avoid-nested-loops
message: Nested loops can cause performance issues
severity: warning
language: javascript
rule:
  pattern: |
    for ($$$) {
      for ($$$) {
        $$$
      }
    }
"#;

    let style_rule = r#"
id: consistent-naming
message: Use camelCase for variable names
severity: info
language: javascript
rule:
  kind: identifier
  regex: "^[a-z][a-zA-Z0-9]*$"
"#;

    fs::write(rules_dir1.join("no-eval.yaml"), security_rule).unwrap();
    fs::write(rules_dir2.join("avoid-nested-loops.yaml"), performance_rule).unwrap();
    fs::write(rules_dir3.join("consistent-naming.yaml"), style_rule).unwrap();

    // Create service with a custom rules directory to avoid picking up default .ast-grep-rules
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        rules_directory: temp_dir.path().join("primary-rules"), // Set a non-existent primary directory
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // List all rules
    let list_param = ListRulesParam {
        language: Some("javascript".to_string()),
        severity: None,
    };

    let rules = service.list_rules(list_param).await.unwrap();
    println!("Found {} rules:", rules.rules.len());
    for rule in &rules.rules {
        println!("  - {} from {}", rule.id, rule.file_path);
    }
    assert_eq!(rules.rules.len(), 3);

    // Verify each rule can be retrieved correctly
    let rule_ids = vec!["no-eval", "avoid-nested-loops", "consistent-naming"];
    for rule_id in rule_ids {
        let get_param = GetRuleParam {
            rule_id: rule_id.to_string(),
        };
        let rule = service.get_rule(get_param).await.unwrap();
        assert_eq!(rule.rule_config.id, rule_id);
    }
}

#[tokio::test]
async fn test_rule_directory_order_matters() {
    // Test that the order of directories in sgconfig.yml matters for duplicate resolution

    let temp_dir = TempDir::new().unwrap();

    // Create directories
    let primary_dir = temp_dir.path().join("primary");
    let override_dir = temp_dir.path().join("override");

    fs::create_dir_all(&primary_dir).unwrap();
    fs::create_dir_all(&override_dir).unwrap();

    // Create sgconfig.yml with override directory first
    let sgconfig_content = r#"
ruleDirs:
  - ./override
  - ./primary
"#;
    fs::write(temp_dir.path().join("sgconfig.yml"), sgconfig_content).unwrap();

    // Create same rule in both directories
    let primary_rule = r#"
id: console-check
message: Primary console check
severity: warning
language: javascript
rule:
  pattern: console.$METHOD($$$)
"#;

    let override_rule = r#"
id: console-check
message: Override console check
severity: error
language: javascript
rule:
  pattern: console.$METHOD($$$)
fix: |
  // Removed console.$METHOD
"#;

    fs::write(primary_dir.join("console-check.yaml"), primary_rule).unwrap();
    fs::write(override_dir.join("console-check.yaml"), override_rule).unwrap();

    // Create service with a custom rules directory to avoid picking up default .ast-grep-rules
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        rules_directory: temp_dir.path().join("primary-rules"), // Set a non-existent primary directory
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // Get the rule - should get the override version since it's listed first
    let get_param = GetRuleParam {
        rule_id: "console-check".to_string(),
    };
    let rule = service.get_rule(get_param).await.unwrap();

    // Verify it's the override version
    assert_eq!(
        rule.rule_config.message,
        Some("Override console check".to_string())
    );
    assert_eq!(rule.rule_config.severity, Some("error".to_string()));
    assert!(rule.rule_config.fix.is_some());
    assert!(rule.file_path.contains("override"));
}
