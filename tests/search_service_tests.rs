//! Tests for SearchService
//! 
//! This module contains unit tests for the search service that were originally
//! embedded in the source files.

use ast_grep_mcp::search::SearchService;
use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::rules::RuleEvaluator;
use ast_grep_mcp::types::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn create_test_search_service() -> (SearchService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();

    (
        SearchService::new(config, pattern_matcher, rule_evaluator),
        temp_dir,
    )
}

fn create_test_file(dir: &Path, name: &str, content: &str) {
    let file_path = dir.join(name);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(file_path, content).unwrap();
}

#[tokio::test]
async fn test_search_basic() {
    let (service, _temp_dir) = create_test_search_service();
    let code = r#"
function greet() {
    console.log("Hello");
    console.error("Error");
}
"#;
    let param = SearchParam::new(code, "console.log($VAR)", "javascript");

    let result = service.search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert!(result.matches[0].text.contains("console.log"));
}

#[tokio::test]
async fn test_search_no_matches() {
    let (service, _temp_dir) = create_test_search_service();
    let code = "function test() { alert('hello'); }";
    let param = SearchParam::new(code, "console.log($VAR)", "javascript");

    let result = service.search(param).await.unwrap();
    assert_eq!(result.matches.len(), 0);
}

#[tokio::test]
async fn test_search_multiple_matches() {
    let (service, _temp_dir) = create_test_search_service();
    let code = r#"
console.log("First");
console.log("Second");
console.log("Third");
"#;
    let param = SearchParam::new(code, "console.log($VAR)", "javascript");

    let result = service.search(param).await.unwrap();
    assert_eq!(result.matches.len(), 3);
}

#[tokio::test]
async fn test_search_with_context_lines() {
    let (service, _temp_dir) = create_test_search_service();
    let code = r#"
line1
console.log("target");
line3
line4
"#;
    let param = SearchParam {
        code: code.to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        context_lines: Some(1),
        ..Default::default()
    };

    let result = service.search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert!(result.matches[0].context_before.is_some());
    assert!(result.matches[0].context_after.is_some());
}

#[tokio::test]
async fn test_file_search_basic() {
    let (service, temp_dir) = create_test_search_service();
    
    // Create test files
    create_test_file(temp_dir.path(), "test1.js", "console.log('test1');");
    create_test_file(temp_dir.path(), "test2.js", "console.log('test2');");
    create_test_file(temp_dir.path(), "test3.ts", "console.log('test3');"); // Different extension
    
    let param = FileSearchParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.files.len(), 2); // Only .js files
    assert_eq!(result.total_matches, 2);
}

#[tokio::test]
async fn test_file_search_nested_directories() {
    let (service, temp_dir) = create_test_search_service();
    
    // Create nested structure
    create_test_file(temp_dir.path(), "src/main.js", "console.log('main');");
    create_test_file(temp_dir.path(), "src/utils/helper.js", "console.log('helper');");
    create_test_file(temp_dir.path(), "tests/test.js", "console.log('test');");
    
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.files.len(), 3);
    assert_eq!(result.total_matches, 3);
}

#[tokio::test]
async fn test_file_search_max_results_limit() {
    let (service, temp_dir) = create_test_search_service();
    
    // Create multiple files
    for i in 1..=5 {
        create_test_file(temp_dir.path(), &format!("test{}.js", i), "console.log('test');");
    }
    
    let param = FileSearchParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        max_results: 3,
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert!(result.files.len() <= 3);
    assert!(result.total_matches <= 3);
}

#[tokio::test]
async fn test_search_with_strictness() {
    let (service, _temp_dir) = create_test_search_service();
    let code = "console.log( 'test' );"; // Extra spaces
    
    // Test with smart strictness (should match)
    let param_smart = SearchParam {
        code: code.to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        strictness: Some(MatchStrictness::Smart),
        ..Default::default()
    };

    let result = service.search(param_smart).await.unwrap();
    assert_eq!(result.matches.len(), 1);
}

#[tokio::test]
async fn test_search_with_selector() {
    let (service, _temp_dir) = create_test_search_service();
    let code = r#"
function test() {
    console.log("inside function");
}
console.log("outside function");
"#;
    
    let param = SearchParam {
        code: code.to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        selector: Some("function_declaration".to_string()),
        ..Default::default()
    };

    let result = service.search(param).await.unwrap();
    // Should only match the console.log inside the function
    assert_eq!(result.matches.len(), 1);
    assert!(result.matches[0].text.contains("inside function"));
}

#[tokio::test]
async fn test_search_invalid_language() {
    let (service, _temp_dir) = create_test_search_service();
    let param = SearchParam::new("test", "pattern", "invalid_language");

    let result = service.search(param).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_search_no_files_found() {
    let (service, _temp_dir) = create_test_search_service();
    
    let param = FileSearchParam {
        path_pattern: "*.nonexistent".to_string(),
        pattern: "test".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.files.len(), 0);
    assert_eq!(result.total_matches, 0);
}