use ast_grep_mcp::{
    SearchParam, config::ServiceConfig, pattern::PatternMatcher, rules::RuleEvaluator,
    search::SearchService,
};
use std::path::PathBuf;

#[tokio::test]
async fn test_debug_pattern_matching() {
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

    println!("=== Testing Different Patterns ===");

    // Test 1: Basic pattern
    let param1 = SearchParam {
        code: test_code.to_string(),
        pattern: "..Default::default()".to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(3),
        context_after: Some(1),
        context_lines: None,
    };

    let result1 = search_service.search(param1).await.unwrap();
    println!(
        "Pattern 1 '..Default::default()': {} matches",
        result1.matches.len()
    );

    // Test 2: Pattern with context field
    let param2 = SearchParam {
        code: test_code.to_string(),
        pattern: "context: None,".to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(1),
        context_after: Some(2),
        context_lines: None,
    };

    let result2 = search_service.search(param2).await.unwrap();
    println!(
        "Pattern 2 'context: None,': {} matches",
        result2.matches.len()
    );

    for (i, m) in result2.matches.iter().enumerate() {
        println!("Match {}: {}", i + 1, m.text);
        if let Some(context_after) = &m.context_after {
            println!("Context after:");
            for line in context_after {
                println!("  > {line}");
            }
        }
    }

    // Test 3: Multi-line pattern
    let param3 = SearchParam {
        code: test_code.to_string(),
        pattern: r#"context: None,
    ..Default::default()"#
            .to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(1),
        context_after: Some(1),
        context_lines: None,
    };

    let result3 = search_service.search(param3).await.unwrap();
    println!("Pattern 3 multiline: {} matches", result3.matches.len());

    for (i, m) in result3.matches.iter().enumerate() {
        println!("Match {}: {}", i + 1, m.text);
        if let Some(context_before) = &m.context_before {
            println!("Context before:");
            for line in context_before {
                println!("  < {line}");
            }
        }
        if let Some(context_after) = &m.context_after {
            println!("Context after:");
            for line in context_after {
                println!("  > {line}");
            }
        }
    }

    // Test 4: Different spacing
    let param4 = SearchParam {
        code: test_code.to_string(),
        pattern: r#"    context: None,
    ..Default::default()"#
            .to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(1),
        context_after: Some(1),
        context_lines: None,
    };

    let result4 = search_service.search(param4).await.unwrap();
    println!(
        "Pattern 4 with exact spacing: {} matches",
        result4.matches.len()
    );

    for (i, m) in result4.matches.iter().enumerate() {
        println!("Match {}: {}", i + 1, m.text);
        println!("Variables: {:?}", m.vars);
    }

    // Test 5: Show the actual line content around context
    if let Some(first_match) = result1.matches.first() {
        if let Some(context_before) = &first_match.context_before {
            println!("\n=== Actual line content around ..Default::default() ===");
            for (i, line) in context_before.iter().enumerate() {
                println!("Line {}: '{}'", i + 1, line);
            }
        }
    }
}

#[tokio::test]
async fn test_exact_whitespace_patterns() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let search_service = SearchService::new(config, pattern_matcher, rule_evaluator);

    // Let's examine the exact whitespace in the test code
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

    // Let's see the exact lines
    let lines: Vec<&str> = test_code.lines().collect();
    println!("=== Exact line content ===");
    for (i, line) in lines.iter().enumerate() {
        if line.contains("context: None") || line.contains("..Default::default") {
            println!("Line {}: '{}'", i + 1, line);
            println!("  Length: {}", line.len());
            println!("  Chars: {:?}", line.chars().collect::<Vec<_>>());
        }
    }

    // Now let's try matching with the exact characters
    let context_line = lines
        .iter()
        .find(|line| line.contains("context: None"))
        .unwrap();
    let default_line = lines
        .iter()
        .find(|line| line.contains("..Default::default"))
        .unwrap();

    let exact_pattern = format!("{context_line}\n{default_line}");
    println!("=== Exact pattern to match ===");
    println!("'{exact_pattern}'");

    let param = SearchParam {
        code: test_code.to_string(),
        pattern: exact_pattern,
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(1),
        context_after: Some(1),
        context_lines: None,
    };

    let result = search_service.search(param).await.unwrap();
    println!("Exact pattern matches: {}", result.matches.len());

    for (i, m) in result.matches.iter().enumerate() {
        println!("Match {}: {}", i + 1, m.text);
    }
}
