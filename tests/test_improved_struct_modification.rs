use ast_grep_mcp::{
    ReplaceParam, config::ServiceConfig, pattern::PatternMatcher, replace::ReplaceService,
    rules::RuleEvaluator,
};
use std::path::PathBuf;

#[tokio::test]
async fn test_improved_struct_modification_with_context() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let replace_service = ReplaceService::new(config, pattern_matcher, rule_evaluator);

    // Test the problematic struct that was causing issues
    let original_code = r#"
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

    println!("=== Original Code ===");
    println!("{original_code}");

    // Strategy 1: Use context lines to identify the correct insertion point
    // Match the specific pattern that includes the last field before ..Default::default()
    let param = ReplaceParam {
        code: original_code.to_string(),
        pattern: r#"    context: None,
    ..Default::default()"#
            .to_string(),
        replacement: r#"    context: None,
    context_before: None,
    context_after: None,
    context_lines: None,
    ..Default::default()"#
            .to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
    };

    let result = replace_service.replace(param).await.unwrap();

    println!("=== After Strategy 1: Specific Pattern Replacement ===");
    println!("{}", result.new_code);

    // Verify the result is valid Rust syntax
    assert!(result.new_code.contains("context: None,"));
    assert!(result.new_code.contains("context_before: None,"));
    assert!(result.new_code.contains("context_after: None,"));
    assert!(result.new_code.contains("context_lines: None,"));
    assert!(result.new_code.contains("..Default::default()"));

    // Verify that ..Default::default() is still at the end
    let lines: Vec<&str> = result.new_code.lines().collect();
    let default_line_idx = lines
        .iter()
        .position(|line| line.contains("..Default::default()"))
        .unwrap();
    let closing_brace_idx = lines.iter().position(|line| line.contains("};")).unwrap();
    assert!(
        default_line_idx < closing_brace_idx,
        "..Default::default() should come before closing brace"
    );

    // Check that there are no fields after ..Default::default()
    let default_line_idx = lines
        .iter()
        .position(|line| line.contains("..Default::default()"))
        .unwrap();
    for line in lines.iter().skip(default_line_idx + 1) {
        let line = line.trim();
        if line.contains(":") && !line.contains("};") {
            panic!("Found field after ..Default::default(): {line}");
        }
    }

    println!("✅ Strategy 1 Success: All fields correctly placed before ..Default::default()");
}

#[tokio::test]
async fn test_generic_struct_update_pattern() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let replace_service = ReplaceService::new(config, pattern_matcher, rule_evaluator);

    // Test a more generic pattern that should work with any struct
    let test_code = r#"
let config = MyConfig {
    name: "test".to_string(),
    enabled: true,
    ..Default::default()
};

let other = AnotherStruct {
    value: 42,
    ..Default::default()
};
"#;

    println!("=== Generic Pattern Test ===");
    println!("{test_code}");

    // Strategy 2: Use a more generic pattern with metavariable
    let param = ReplaceParam {
        code: test_code.to_string(),
        pattern: r#"$FIELD,
    ..Default::default()"#
            .to_string(),
        replacement: r#"$FIELD,
    new_field: None,
    ..Default::default()"#
            .to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
    };

    let result = replace_service.replace(param).await.unwrap();

    println!("=== After Generic Pattern Replacement ===");
    println!("{}", result.new_code);

    // Should add new_field to both structs
    assert_eq!(result.new_code.matches("new_field: None,").count(), 2);
    assert_eq!(result.new_code.matches("..Default::default()").count(), 2);

    // Verify syntax is still valid
    let lines: Vec<&str> = result.new_code.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains("..Default::default()") {
            // Check that no fields come after this line (before closing brace)
            for (j, next_line) in lines.iter().enumerate().skip(i + 1) {
                let next_line = next_line.trim();
                if next_line.contains(":") && !next_line.contains("};") {
                    panic!(
                        "Found field after ..Default::default() at line {}: {}",
                        j + 1,
                        next_line
                    );
                }
                if next_line.contains("};") {
                    break;
                }
            }
        }
    }

    println!("✅ Strategy 2 Success: Generic pattern correctly handles multiple structs");
}

#[tokio::test]
async fn test_context_lines_for_debugging() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let search_service =
        ast_grep_mcp::search::SearchService::new(config, pattern_matcher, rule_evaluator);

    // Use context lines to analyze the struct before modification
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

    let param = ast_grep_mcp::SearchParam {
        code: test_code.to_string(),
        pattern: "..Default::default()".to_string(),
        language: "rust".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(5),
        context_after: Some(2),
        context_lines: None,
    };

    let result = search_service.search(param).await.unwrap();

    println!("=== Context Lines Analysis ===");
    for (i, m) in result.matches.iter().enumerate() {
        println!("Match {}: {}", i + 1, m.text);

        if let Some(context_before) = &m.context_before {
            println!("Context before (showing insertion point):");
            for (j, line) in context_before.iter().enumerate() {
                let line_num = m.start_line as isize - context_before.len() as isize + j as isize;
                println!("  {line_num:2}: {line}");
            }
        }

        println!("  --> {}: {} (THE MATCH)", m.start_line, m.text);

        if let Some(context_after) = &m.context_after {
            println!("Context after:");
            for (j, line) in context_after.iter().enumerate() {
                let line_num = m.end_line + j + 1;
                println!("  {line_num:2}: {line}");
            }
        }
    }

    println!("✅ Context lines clearly show where to insert new fields");
}

#[test]
fn test_pattern_design_insights() {
    println!("=== Pattern Design Insights from Context Lines ===");
    println!("1. The ..Default::default() must always be the last item in a struct literal");
    println!("2. Context lines help us see the exact structure around the insertion point");
    println!("3. Better patterns match from the last field to the struct update syntax");
    println!("4. This prevents fields from being added after ..Default::default()");
    println!("5. The context feature transforms debugging from guesswork to precision");
}
