use ast_grep_mcp::ReplaceParam;
use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::replace::ReplaceService;
use ast_grep_mcp::rules::RuleEvaluator;
use std::path::PathBuf;

#[tokio::test]
async fn test_adding_fields_to_struct_with_default_expansion() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let replace_service = ReplaceService::new(config, pattern_matcher, rule_evaluator);

    // Original code with struct update syntax
    let original_code = r#"
fn create_param() {
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
}
"#;

    // Try to add new fields - this is what went wrong
    let param = ReplaceParam {
        code: original_code.to_string(),
        pattern: r#"FileSearchParam {
    $$$FIELDS
}"#
        .to_string(),
        replacement: r#"FileSearchParam {
    $$$FIELDS
    selector: None,
    context: None,
}"#
        .to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
    };

    let result = replace_service.replace(param).await.unwrap();

    println!("Original code:");
    println!("{original_code}");
    println!("\nResult code:");
    println!("{}", result.new_code);

    // The result will likely be invalid Rust syntax with fields after ..Default::default()
    assert!(result.new_code.contains("..Default::default()"));

    // This would be invalid:
    // ..Default::default()
    // selector: None,
    // context: None,
}

#[tokio::test]
async fn test_correct_way_to_add_fields_before_default() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let replace_service = ReplaceService::new(config, pattern_matcher, rule_evaluator);

    let original_code = r#"
fn create_param() {
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
}
"#;

    // Approach 1: Match the specific pattern before ..Default::default()
    let param = ReplaceParam {
        code: original_code.to_string(),
        pattern: r#"        ..Default::default()"#.to_string(),
        replacement: r#"        selector: None,
        context: None,
        ..Default::default()"#
            .to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
    };

    let result = replace_service.replace(param).await.unwrap();

    println!("\nCorrected code:");
    println!("{}", result.new_code);

    // Should have fields before ..Default::default()
    assert!(result.new_code.contains("selector: None,"));
    assert!(result.new_code.contains("context: None,"));
    assert!(result.new_code.contains("..Default::default()"));

    // Check that the order is correct
    let selector_pos = result.new_code.find("selector: None").unwrap();
    let default_pos = result.new_code.find("..Default::default()").unwrap();
    assert!(
        selector_pos < default_pos,
        "Fields should come before ..Default::default()"
    );
}

#[tokio::test]
async fn test_ast_aware_struct_field_insertion() {
    // This test explores whether we can use AST-aware patterns to correctly handle struct updates

    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let _replace_service = ReplaceService::new(config, pattern_matcher, rule_evaluator);

    let original_code = r#"
let param1 = MyStruct {
    field1: value1,
    ..Default::default()
};

let param2 = MyStruct {
    field1: value1,
    field2: value2,
};
"#;

    // Try to match struct expressions and handle them differently based on presence of ..
    // This would require more sophisticated pattern matching

    println!("\nOriginal struct code:");
    println!("{original_code}");

    // For now, ast-grep doesn't have a built-in way to handle this case elegantly
    // We would need to:
    // 1. Detect if a struct has ..Default::default() or similar
    // 2. Insert new fields before it, not after
    // 3. Handle trailing commas correctly
}

#[test]
fn test_rust_struct_update_syntax_rules() {
    // Document the Rust syntax rules for struct updates

    // Valid: fields before struct update
    let _valid = r#"
    MyStruct {
        field1: value1,
        field2: value2,
        ..other_struct
    }
    "#;

    // Invalid: fields after struct update
    let _invalid = r#"
    MyStruct {
        field1: value1,
        ..other_struct,
        field2: value2,  // ERROR: expected `}`, found `field2`
    }
    "#;

    // The struct update syntax (..) must always be the last item
    println!("Rust requires struct update syntax to be the last item in struct literal");
}
