//! Test RuleStorage with multiple rule directories

use ast_grep_mcp::rules::RuleStorage;
use ast_grep_mcp::rules::types::{CreateRuleParam, DeleteRuleParam, GetRuleParam, ListRulesParam};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_rule_storage_single_directory() {
    let temp_dir = TempDir::new().unwrap();
    let rules_dir = temp_dir.path().to_path_buf();

    let storage = RuleStorage::new(rules_dir.clone());

    // Create a rule
    let rule_config = r#"
id: test-rule
message: Test rule
language: javascript
rule:
  pattern: console.log($ARG)
"#;

    let create_param = CreateRuleParam {
        rule_config: rule_config.to_string(),
        overwrite: false,
    };

    let result = storage.create_rule(create_param).await.unwrap();
    assert_eq!(result.rule_id, "test-rule");
    assert!(result.created);

    // List rules
    let list_param = ListRulesParam {
        language: Some("javascript".to_string()),
        severity: None,
    };

    let list_result = storage.list_rules(list_param).await.unwrap();
    assert_eq!(list_result.rules.len(), 1);
    assert_eq!(list_result.rules[0].id, "test-rule");
}

#[tokio::test]
async fn test_rule_storage_multiple_directories() {
    let temp_dir = TempDir::new().unwrap();
    let rules_dir1 = temp_dir.path().join("rules1");
    let rules_dir2 = temp_dir.path().join("rules2");
    let rules_dir3 = temp_dir.path().join("rules3");

    // Create directories
    fs::create_dir_all(&rules_dir1).unwrap();
    fs::create_dir_all(&rules_dir2).unwrap();
    fs::create_dir_all(&rules_dir3).unwrap();

    // Create rules in different directories
    let rule1 = r#"
id: rule-from-dir1
message: Rule from directory 1
language: javascript
rule:
  pattern: console.log($ARG)
"#;

    let rule2 = r#"
id: rule-from-dir2
message: Rule from directory 2
language: rust
rule:
  pattern: println!($ARG)
"#;

    let rule3 = r#"
id: rule-from-dir3
message: Rule from directory 3
language: javascript
rule:
  pattern: console.error($ARG)
"#;

    // Write rules to different directories
    fs::write(rules_dir1.join("rule-from-dir1.yaml"), rule1).unwrap();
    fs::write(rules_dir2.join("rule-from-dir2.yaml"), rule2).unwrap();
    fs::write(rules_dir3.join("rule-from-dir3.yaml"), rule3).unwrap();

    // Create storage with multiple directories
    let storage = RuleStorage::with_directories(vec![
        rules_dir1.clone(),
        rules_dir2.clone(),
        rules_dir3.clone(),
    ]);

    // List all rules
    let list_param = ListRulesParam {
        language: None,
        severity: None,
    };

    let list_result = storage.list_rules(list_param).await.unwrap();
    assert_eq!(list_result.rules.len(), 3);

    // Filter by language
    let js_list_param = ListRulesParam {
        language: Some("javascript".to_string()),
        severity: None,
    };

    let js_list_result = storage.list_rules(js_list_param).await.unwrap();
    assert_eq!(js_list_result.rules.len(), 2);

    // Get specific rules
    let get_param1 = GetRuleParam {
        rule_id: "rule-from-dir1".to_string(),
    };
    let get_result1 = storage.get_rule(get_param1).await.unwrap();
    assert_eq!(get_result1.rule_config.id, "rule-from-dir1");

    let get_param2 = GetRuleParam {
        rule_id: "rule-from-dir2".to_string(),
    };
    let get_result2 = storage.get_rule(get_param2).await.unwrap();
    assert_eq!(get_result2.rule_config.id, "rule-from-dir2");
}

#[tokio::test]
async fn test_rule_storage_duplicate_handling() {
    let temp_dir = TempDir::new().unwrap();
    let rules_dir1 = temp_dir.path().join("rules1");
    let rules_dir2 = temp_dir.path().join("rules2");

    // Create directories
    fs::create_dir_all(&rules_dir1).unwrap();
    fs::create_dir_all(&rules_dir2).unwrap();

    // Create the same rule ID in different directories
    let rule = r#"
id: duplicate-rule
message: This rule exists in multiple directories
language: javascript
rule:
  pattern: console.log($ARG)
"#;

    // Write the same rule to both directories
    fs::write(rules_dir1.join("duplicate-rule.yaml"), rule).unwrap();
    fs::write(rules_dir2.join("duplicate-rule.yaml"), rule).unwrap();

    // Create storage with multiple directories
    let storage = RuleStorage::with_directories(vec![rules_dir1.clone(), rules_dir2.clone()]);

    // List rules should only show one instance
    let list_param = ListRulesParam {
        language: None,
        severity: None,
    };

    let list_result = storage.list_rules(list_param).await.unwrap();
    assert_eq!(list_result.rules.len(), 1);
    assert_eq!(list_result.rules[0].id, "duplicate-rule");
}

#[tokio::test]
async fn test_rule_storage_delete_from_multiple_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let rules_dir1 = temp_dir.path().join("rules1");
    let rules_dir2 = temp_dir.path().join("rules2");

    // Create directories
    fs::create_dir_all(&rules_dir1).unwrap();
    fs::create_dir_all(&rules_dir2).unwrap();

    // Create a rule in the first directory
    let rule = r#"
id: deletable-rule
message: Rule to be deleted
language: javascript
rule:
  pattern: console.log($ARG)
"#;

    fs::write(rules_dir1.join("deletable-rule.yaml"), rule).unwrap();

    // Create storage with multiple directories
    let storage = RuleStorage::with_directories(vec![rules_dir1.clone(), rules_dir2.clone()]);

    // Delete the rule
    let delete_param = DeleteRuleParam {
        rule_id: "deletable-rule".to_string(),
    };

    let delete_result = storage.delete_rule(delete_param).await.unwrap();
    assert!(delete_result.deleted);
    assert_eq!(delete_result.rule_id, "deletable-rule");

    // Verify the file is deleted
    assert!(!rules_dir1.join("deletable-rule.yaml").exists());
}

#[tokio::test]
async fn test_rule_storage_create_in_primary_dir() {
    let temp_dir = TempDir::new().unwrap();
    let rules_dir1 = temp_dir.path().join("primary");
    let rules_dir2 = temp_dir.path().join("secondary");

    // Create storage with multiple directories
    let storage = RuleStorage::with_directories(vec![rules_dir1.clone(), rules_dir2.clone()]);

    // Create a new rule
    let rule_config = r#"
id: new-rule
message: New rule created
language: javascript
rule:
  pattern: console.log($ARG)
"#;

    let create_param = CreateRuleParam {
        rule_config: rule_config.to_string(),
        overwrite: false,
    };

    let result = storage.create_rule(create_param).await.unwrap();
    assert!(result.created);

    // Verify the rule was created in the primary directory
    assert!(rules_dir1.join("new-rule.yaml").exists());
    assert!(!rules_dir2.join("new-rule.yaml").exists());
}
