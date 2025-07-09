use ast_grep_mcp::{
    SearchParam, config::ServiceConfig, pattern::PatternMatcher, rules::RuleEvaluator,
    search::SearchService,
};
use std::path::PathBuf;

#[tokio::test]
async fn test_simple_search() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let search_service = SearchService::new(config, pattern_matcher, rule_evaluator);

    let test_code = r#"
let param = FileSearchParam {
    path_pattern: "**/*.js".to_string(),
    pattern: "console.log($VAR)".to_string(),
    language: "javascript".to_string(),
    max_results: 10,
    max_file_size: 1024 * 1024,
    cursor: None,
    strictness: None,
    selector: None,
    context: None,
    ..Default::default()
};
"#;

    println!("=== Testing Simple Patterns ===");

    // Test 1: Simple string literal
    let param1 = SearchParam {
        code: test_code.to_string(),
        pattern: "FileSearchParam".to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(1),
        context_after: Some(1),
        context_lines: None,
    };

    let result1 = search_service.search(param1).await.unwrap();
    println!(
        "Pattern 'FileSearchParam': {} matches",
        result1.matches.len()
    );

    for (i, m) in result1.matches.iter().enumerate() {
        println!("Match {}: {}", i + 1, m.text);
        println!("  Line: {}", m.start_line);
    }

    // Test 2: Simple variable pattern
    let param2 = SearchParam {
        code: test_code.to_string(),
        pattern: "let $VAR = FileSearchParam".to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(1),
        context_after: Some(1),
        context_lines: None,
    };

    let result2 = search_service.search(param2).await.unwrap();
    println!(
        "Pattern 'let $VAR = FileSearchParam': {} matches",
        result2.matches.len()
    );

    for (i, m) in result2.matches.iter().enumerate() {
        println!("Match {}: {}", i + 1, m.text);
        println!("  Variables: {:?}", m.vars);
    }

    // Test 3: Try to match Default
    let param3 = SearchParam {
        code: test_code.to_string(),
        pattern: "Default".to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(1),
        context_after: Some(1),
        context_lines: None,
    };

    let result3 = search_service.search(param3).await.unwrap();
    println!("Pattern 'Default': {} matches", result3.matches.len());

    for (i, m) in result3.matches.iter().enumerate() {
        println!("Match {}: {}", i + 1, m.text);
        println!("  Line: {}", m.start_line);
    }
}
