//! Test that warnings are emitted for duplicate rule IDs

use ast_grep_mcp::{ListRulesParam, ast_grep_service::AstGrepService, config::ServiceConfig};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_duplicate_rule_id_warning() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple rule directories
    let rules_dir1 = temp_dir.path().join("rules1");
    let rules_dir2 = temp_dir.path().join("rules2");
    let rules_dir3 = temp_dir.path().join("rules3");

    fs::create_dir_all(&rules_dir1).unwrap();
    fs::create_dir_all(&rules_dir2).unwrap();
    fs::create_dir_all(&rules_dir3).unwrap();

    // Create sgconfig.yml
    let sgconfig_content = r#"
ruleDirs:
  - ./rules1
  - ./rules2
  - ./rules3
"#;
    fs::write(temp_dir.path().join("sgconfig.yml"), sgconfig_content).unwrap();

    // Create rules with duplicate IDs
    let rule1 = r#"
id: duplicate-check
message: First version from rules1
language: javascript
rule:
  pattern: console.log($ARG)
"#;

    let rule2 = r#"
id: duplicate-check
message: Second version from rules2
language: javascript
rule:
  pattern: console.error($ARG)
"#;

    let rule3 = r#"
id: unique-rule
message: This rule has a unique ID
language: javascript
rule:
  pattern: console.debug($ARG)
"#;

    let rule4 = r#"
id: duplicate-check
message: Third version from rules3
language: javascript
rule:
  pattern: console.warn($ARG)
"#;

    fs::write(rules_dir1.join("duplicate-check.yaml"), rule1).unwrap();
    fs::write(rules_dir2.join("duplicate-check.yaml"), rule2).unwrap();
    fs::write(rules_dir2.join("unique-rule.yaml"), rule3).unwrap();
    fs::write(rules_dir3.join("duplicate-check.yaml"), rule4).unwrap();

    // Create service
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        rules_directory: temp_dir.path().join("primary-rules"),
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // List rules - this should trigger warnings to stderr
    let list_param = ListRulesParam {
        language: None,
        severity: None,
    };

    let rules = service.list_rules(list_param).await.unwrap();

    // Verify only unique rules are returned
    assert_eq!(rules.rules.len(), 2); // duplicate-check (first one) and unique-rule

    // Verify the first occurrence won
    let duplicate_rule = rules
        .rules
        .iter()
        .find(|r| r.id == "duplicate-check")
        .expect("Should find duplicate-check rule");

    assert!(duplicate_rule.file_path.contains("rules1"));
    assert_eq!(
        duplicate_rule.message,
        Some("First version from rules1".to_string())
    );
}

#[tokio::test]
async fn test_no_warning_for_unique_rules() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple rule directories
    let rules_dir1 = temp_dir.path().join("rules1");
    let rules_dir2 = temp_dir.path().join("rules2");

    fs::create_dir_all(&rules_dir1).unwrap();
    fs::create_dir_all(&rules_dir2).unwrap();

    // Create sgconfig.yml
    let sgconfig_content = r#"
ruleDirs:
  - ./rules1
  - ./rules2
"#;
    fs::write(temp_dir.path().join("sgconfig.yml"), sgconfig_content).unwrap();

    // Create rules with unique IDs
    let rule1 = r#"
id: rule-one
message: Rule from rules1
language: javascript
rule:
  pattern: console.log($ARG)
"#;

    let rule2 = r#"
id: rule-two
message: Rule from rules2
language: javascript
rule:
  pattern: console.error($ARG)
"#;

    fs::write(rules_dir1.join("rule-one.yaml"), rule1).unwrap();
    fs::write(rules_dir2.join("rule-two.yaml"), rule2).unwrap();

    // Create service
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        rules_directory: temp_dir.path().join("primary-rules"),
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // List rules - should not trigger any warnings
    let list_param = ListRulesParam {
        language: None,
        severity: None,
    };

    let rules = service.list_rules(list_param).await.unwrap();

    // Verify both rules are returned
    assert_eq!(rules.rules.len(), 2);

    let rule_ids: Vec<&str> = rules.rules.iter().map(|r| r.id.as_str()).collect();
    assert!(rule_ids.contains(&"rule-one"));
    assert!(rule_ids.contains(&"rule-two"));
}
