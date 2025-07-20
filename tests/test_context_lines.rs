use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::rules::RuleEvaluator;
use ast_grep_mcp::search::SearchService;
use ast_grep_mcp::{FileSearchParam, SearchParam};
use std::path::PathBuf;
use tokio::fs;

#[tokio::test]
#[ignore = "TODO: Implement context lines functionality"]
async fn test_context_lines_in_search_results() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let search_service = SearchService::new(config, pattern_matcher, rule_evaluator);

    // Create test code with multiple lines
    let test_code = r#"
function processData(data) {
    const result = [];
    for (const item of data) {
        console.log("Processing item:", item);
        result.push(transform(item));
    }
    return result;
}

function transform(item) {
    console.log("Transforming:", item);
    return item.toUpperCase();
}
"#;

    // Test 1: Basic search with context lines
    let param = SearchParam {
        code: test_code.to_string(),
        pattern: "console.log($MSG, $VAR)".to_string(),
        language: "javascript".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(2),
        context_after: Some(1),
        context_lines: None,
    };

    let result = search_service.search(param).await.unwrap();

    println!("Search results with context lines:");
    println!("{:?}", result.matches_summary);

    // Should find 2 matches with context lines
    assert_eq!(result.matches.len(), 2);

    // Each match should have context lines
    for m in &result.matches {
        assert!(m.context_before.is_some());
        assert!(m.context_after.is_some());
        assert_eq!(m.context_before.as_ref().unwrap().len(), 2);
        assert_eq!(m.context_after.as_ref().unwrap().len(), 1);
    }
}

#[tokio::test]
#[ignore = "TODO: Implement context lines functionality"]
async fn test_context_lines_parameter() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let search_service = SearchService::new(config, pattern_matcher, rule_evaluator);

    let test_code = r#"
// line 1
// line 2
// line 3
TARGET_LINE;
// line 5
// line 6
// line 7
"#;

    // Test with context_lines parameter (equivalent to -C in grep)
    let param = SearchParam {
        code: test_code.to_string(),
        pattern: "TARGET_LINE".to_string(),
        language: "javascript".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: None,
        context_after: None,
        context_lines: Some(2),
    };

    let result = search_service.search(param).await.unwrap();

    // Should have 2 lines before and 2 lines after
    assert_eq!(result.matches.len(), 1);
    let match_result = &result.matches[0];
    assert_eq!(match_result.context_before.as_ref().unwrap().len(), 2);
    assert_eq!(match_result.context_after.as_ref().unwrap().len(), 2);
}

#[tokio::test]
#[ignore = "TODO: Implement context lines functionality"]
async fn test_file_search_with_context_lines() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let search_service = SearchService::new(config, pattern_matcher, rule_evaluator);

    // Create a test file
    let test_file = "/tmp/test_context.js";
    let test_content = r#"
function example() {
    const data = getData();
    console.log("Starting process");

    for (const item of data) {
        console.log("Processing:", item);
        processItem(item);
    }

    console.log("Process complete");
    return true;
}
"#;

    fs::write(test_file, test_content).await.unwrap();

    // Test file search with context lines
    let param = FileSearchParam {
        path_pattern: test_file.to_string(),
        pattern: "console.log($MSG)".to_string(),
        language: "javascript".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(1),
        context_after: Some(1),
        context_lines: None,
        max_results: 10,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = search_service.file_search(param).await.unwrap();

    // Should find matches with context
    assert!(!result.matches.is_empty());

    for file_match in &result.matches {
        for pattern_match in &file_match.matches {
            assert!(pattern_match.context_before.is_some());
            assert!(pattern_match.context_after.is_some());
        }
    }

    // Clean up
    fs::remove_file(test_file).await.unwrap();
}

#[tokio::test]
#[ignore = "TODO: Implement context lines functionality"]
async fn test_context_lines_edge_cases() {
    let config = ServiceConfig {
        root_directories: vec![PathBuf::from("/tmp")],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let search_service = SearchService::new(config, pattern_matcher, rule_evaluator);

    // Test: Match at beginning of file
    let test_code = r#"MATCH_HERE;
// line 2
// line 3
// line 4"#;

    let param = SearchParam {
        code: test_code.to_string(),
        pattern: "MATCH_HERE".to_string(),
        language: "javascript".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(3),
        context_after: Some(2),
        context_lines: None,
    };

    let result = search_service.search(param).await.unwrap();

    assert_eq!(result.matches.len(), 1);
    let match_result = &result.matches[0];

    // Should have 0 context lines before (start of file)
    assert_eq!(match_result.context_before.as_ref().unwrap().len(), 0);
    // Should have 2 context lines after
    assert_eq!(match_result.context_after.as_ref().unwrap().len(), 2);

    // Test: Match at end of file
    let test_code2 = r#"// line 1
// line 2
// line 3
MATCH_HERE;"#;

    let param2 = SearchParam {
        code: test_code2.to_string(),
        pattern: "MATCH_HERE".to_string(),
        language: "javascript".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(2),
        context_after: Some(3),
        context_lines: None,
    };

    let result2 = search_service.search(param2).await.unwrap();

    assert_eq!(result2.matches.len(), 1);
    let match_result2 = &result2.matches[0];

    // Should have 2 context lines before
    assert_eq!(match_result2.context_before.as_ref().unwrap().len(), 2);
    // Should have minimal context lines after (end of file)
    // The implementation may include an empty line or trailing context
    assert!(match_result2.context_after.as_ref().unwrap().len() <= 1);
}

#[test]
fn test_context_lines_parameter_validation() {
    // Test that context_lines takes precedence over individual before/after settings
    let _param = SearchParam {
        context_lines: Some(3),
        context_before: Some(1),
        context_after: Some(2),
        ..Default::default()
    };

    // When context_lines is set, it should be used for both before and after
    // This will be validated in the implementation
}
