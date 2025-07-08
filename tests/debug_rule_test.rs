use ast_grep_mcp::{RuleValidateParam, ast_grep_service::AstGrepService};

#[tokio::test]
async fn test_rule_validation() {
    let service = AstGrepService::new();

    // Test with a simple rule
    let rule_config = r#"
id: test-console
language: javascript
rule:
  pattern: console.log($ARGS)
"#;

    let param = RuleValidateParam {
        rule_config: rule_config.to_string(),
        test_code: Some("console.log('test');".to_string()),
    };

    let result = service.validate_rule(param).await.unwrap();

    println!("Rule validation result:");
    println!("  Valid: {}", result.valid);
    println!("  Errors: {:?}", result.errors);
    if let Some(test_results) = &result.test_results {
        println!("  Test matches: {}", test_results.matches_found);
    }

    assert!(result.valid, "Rule should be valid");
}
