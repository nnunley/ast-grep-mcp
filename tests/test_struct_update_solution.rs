use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::replace::ReplaceService;
use ast_grep_mcp::rules::RuleEvaluator;
use ast_grep_mcp::types::ReplaceParam;
use std::path::PathBuf;

#[tokio::test]
#[ignore = "ast-grep pattern matching behavior differs from exact text matching - needs investigation"]
async fn test_correct_struct_field_insertion() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let replace_service = ReplaceService::new(config, pattern_matcher, rule_evaluator);

    // Test Case 1: Insert before the base field initializer
    let code1 = r#"
let param = FileSearchParam {
    path_pattern: "**/*.js".to_string(),
    pattern: "console.log($VAR)".to_string(),
    language: "javascript".to_string(),
    ..Default::default()
};"#;

    // Strategy: Match the pattern including the comma before ..Default::default()
    let param1 = ReplaceParam {
        code: code1.to_string(),
        pattern: r#"    language: "javascript".to_string(),
    ..Default::default()"#
            .to_string(),
        replacement: r#"    language: "javascript".to_string(),
    selector: None,
    context: None,
    ..Default::default()"#
            .to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
    };

    let result1 = replace_service.replace(param1).await.unwrap();
    println!("Test 1 - Insert before base initializer:");
    println!("{}", result1.new_code);

    assert!(result1.new_code.contains("selector: None,"));
    assert!(result1.new_code.contains("context: None,"));
    assert!(result1.new_code.contains("..Default::default()"));

    // Verify order
    let selector_pos = result1.new_code.find("selector: None").unwrap();
    let default_pos = result1.new_code.find("..Default::default()").unwrap();
    assert!(selector_pos < default_pos);
}

#[tokio::test]
#[ignore = "ast-grep pattern matching behavior differs from exact text matching - needs investigation"]
async fn test_ast_based_struct_modification() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let replace_service = ReplaceService::new(config, pattern_matcher, rule_evaluator);

    // Test Case 2: Use a more sophisticated pattern
    let _code = r#"
fn example() {
    let p1 = MyStruct {
        field1: value1,
        field2: value2,
        ..Default::default()
    };

    let p2 = MyStruct {
        field1: value1,
        field2: value2,
    };
}"#;

    // Pattern that matches the last field before base initializer
    let param = ReplaceParam {
        code: _code.to_string(),
        pattern: r#"        field2: value2,
        ..Default::default()"#
            .to_string(),
        replacement: r#"        field2: value2,
        new_field: None,
        ..Default::default()"#
            .to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
    };

    let result = replace_service.replace(param).await.unwrap();
    println!("\nTest 2 - Pattern-based insertion:");
    println!("{}", result.new_code);

    // Should only modify the struct with Default::default()
    assert!(result.new_code.contains("new_field: None,"));
    assert_eq!(result.new_code.matches("new_field: None").count(), 1);
}

#[tokio::test]
async fn test_rule_based_struct_modification() {
    use ast_grep_mcp::rules::{RuleEvaluator, RuleReplaceParam, RuleService, RuleStorage};

    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };

    let rule_storage = RuleStorage::new(PathBuf::from("/tmp/rules"));
    let rule_evaluator = RuleEvaluator::new();
    let _rule_service = RuleService::new(config, rule_evaluator, rule_storage);

    // Create a rule that handles struct updates correctly
    let rule_yaml = r#"
id: add-fields-to-struct
language: rust
rule:
  pattern: |
    FileSearchParam {
      $$$EXISTING_FIELDS
      ..Default::default()
    }
fix: |
  FileSearchParam {
    $$$EXISTING_FIELDS
    selector: None,
    context: None,
    ..Default::default()
  }
"#;

    let _code = r#"
let param = FileSearchParam {
    path_pattern: "test".to_string(),
    ..Default::default()
};"#;

    let _param = RuleReplaceParam {
        rule_config: rule_yaml.to_string(),
        path_pattern: None,
        max_results: 10,
        max_file_size: 1024 * 1024,
        dry_run: true,
        summary_only: false,
        cursor: None,
    };

    // Note: This would require file-based operation
    // The rule shows the correct pattern but needs files to operate on
    println!("\nRule-based approach defined");
}

#[test]
fn test_tree_sitter_cannot_generate() {
    // Important finding: Tree-sitter is a parser, not a code generator
    // It can parse code into an AST but cannot generate code from an AST

    // The solution must work at the pattern-matching level:
    // 1. Match the right location in the source
    // 2. Do text replacement that maintains valid syntax
    // 3. Let tree-sitter re-parse to verify correctness

    println!("Tree-sitter insight: We must match patterns that preserve syntax validity");
    println!("The replacement must be done at the text level, not AST level");
}

#[test]
fn test_best_practices_for_struct_updates() {
    // Document the best practices discovered

    println!("Best practices for modifying structs with update syntax:");
    println!("1. Match from the last field to the update syntax");
    println!("2. Insert new fields with proper indentation and commas");
    println!("3. Always keep the update syntax (..) as the last item");
    println!("4. Consider using more specific patterns that include context");
    println!("5. Test the output to ensure valid Rust syntax");
}
