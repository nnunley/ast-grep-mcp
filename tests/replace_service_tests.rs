//! Tests for ReplaceService
//! 
//! This module contains unit tests for the replace service that were originally
//! embedded in the source files.

use ast_grep_mcp::replace::ReplaceService;
use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::rules::RuleEvaluator;
use ast_grep_mcp::types::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn create_test_replace_service() -> (ReplaceService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();

    (
        ReplaceService::new(config, pattern_matcher, rule_evaluator),
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
async fn test_replace_basic() {
    let (service, _temp_dir) = create_test_replace_service();
    let code = "console.log('Hello'); console.log('World');";
    
    let param = ReplaceParam::new(
        code,
        "console.log($VAR)",
        "console.warn($VAR)",
        "javascript",
    );

    let result = service.replace(param).await.unwrap();
    assert_eq!(result.matches.len(), 2);
    assert_eq!(result.new_code, "console.warn('Hello'); console.warn('World');");
}

#[tokio::test]
async fn test_replace_no_matches() {
    let (service, _temp_dir) = create_test_replace_service();
    let code = "alert('Hello');";
    
    let param = ReplaceParam::new(
        code,
        "console.log($VAR)",
        "console.warn($VAR)",
        "javascript",
    );

    let result = service.replace(param).await.unwrap();
    assert_eq!(result.matches.len(), 0);
    assert_eq!(result.new_code, code); // Should be unchanged
}

#[tokio::test]
async fn test_replace_with_multiple_variables() {
    let (service, _temp_dir) = create_test_replace_service();
    let code = "function test(a, b) { return a + b; }";
    
    let param = ReplaceParam::new(
        code,
        "function $NAME($PARAMS) { $BODY }",
        "const $NAME = ($PARAMS) => { $BODY }",
        "javascript",
    );

    let result = service.replace(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert!(result.new_code.contains("const test = (a, b) => {"));
}

#[tokio::test]
async fn test_file_replace_dry_run() {
    let (service, temp_dir) = create_test_replace_service();
    
    // Create test files
    create_test_file(temp_dir.path(), "test1.js", "console.log('test1');");
    create_test_file(temp_dir.path(), "test2.js", "console.log('test2');");
    
    let param = FileReplaceParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        replacement: "console.warn($VAR)".to_string(),
        language: "javascript".to_string(),
        dry_run: true,
        ..Default::default()
    };

    let result = service.file_replace(param).await.unwrap();
    assert_eq!(result.files_with_changes, 2);
    assert_eq!(result.total_changes, 2);
    
    // Verify files weren't actually changed
    let content1 = fs::read_to_string(temp_dir.path().join("test1.js")).unwrap();
    assert_eq!(content1, "console.log('test1');");
}

#[tokio::test]
async fn test_file_replace_actual_changes() {
    let (service, temp_dir) = create_test_replace_service();
    
    // Create test files
    create_test_file(temp_dir.path(), "test1.js", "console.log('test1');");
    create_test_file(temp_dir.path(), "test2.js", "console.log('test2');");
    
    let param = FileReplaceParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        replacement: "console.warn($VAR)".to_string(),
        language: "javascript".to_string(),
        dry_run: false,
        ..Default::default()
    };

    let result = service.file_replace(param).await.unwrap();
    assert_eq!(result.files_with_changes, 2);
    assert_eq!(result.total_changes, 2);
    
    // Verify files were actually changed
    let content1 = fs::read_to_string(temp_dir.path().join("test1.js")).unwrap();
    assert_eq!(content1, "console.warn('test1');");
}

#[tokio::test]
async fn test_file_replace_with_backup() {
    let (service, temp_dir) = create_test_replace_service();
    
    create_test_file(temp_dir.path(), "test.js", "console.log('test');");
    
    let param = FileReplaceParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        replacement: "console.warn($VAR)".to_string(),
        language: "javascript".to_string(),
        dry_run: false,
        ..Default::default()
    };

    let _result = service.file_replace(param).await.unwrap();
    
    // Check that the file was modified
    let content = fs::read_to_string(temp_dir.path().join("test.js")).unwrap();
    assert_eq!(content, "console.warn('test');");
}

#[tokio::test]
async fn test_file_replace_max_results_limit() {
    let (service, temp_dir) = create_test_replace_service();
    
    // Create multiple files with multiple matches each
    for i in 1..=3 {
        create_test_file(
            temp_dir.path(), 
            &format!("test{}.js", i), 
            "console.log('a'); console.log('b');"
        );
    }
    
    let param = FileReplaceParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        replacement: "console.warn($VAR)".to_string(),
        language: "javascript".to_string(),
        dry_run: true,
        max_results: 3, // Limit to 3 total matches
        ..Default::default()
    };

    let result = service.file_replace(param).await.unwrap();
    assert!(result.total_changes <= 3);
}

#[tokio::test]
async fn test_replace_with_context() {
    let (service, _temp_dir) = create_test_replace_service();
    let code = r#"
line1
console.log('target');
line3
"#;
    
    let param = ReplaceParam {
        code: code.to_string(),
        pattern: "console.log($VAR)".to_string(),
        replacement: "console.warn($VAR)".to_string(),
        language: "javascript".to_string(),
    };

    let result = service.replace(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert!(result.new_code.contains("console.warn('target');"));
}

#[tokio::test]
async fn test_replace_invalid_language() {
    let (service, _temp_dir) = create_test_replace_service();
    
    let param = ReplaceParam::new(
        "test code",
        "pattern",
        "replacement", 
        "invalid_language",
    );

    let result = service.replace(param).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_replace_no_files_found() {
    let (service, _temp_dir) = create_test_replace_service();
    
    let param = FileReplaceParam {
        path_pattern: "*.nonexistent".to_string(),
        pattern: "test".to_string(),
        replacement: "replacement".to_string(),
        language: "javascript".to_string(),
        dry_run: true,
        ..Default::default()
    };

    let result = service.file_replace(param).await.unwrap();
    assert_eq!(result.files_with_changes, 0);
    assert_eq!(result.total_changes, 0);
}

#[tokio::test]
async fn test_replace_summary_only_mode() {
    let (service, temp_dir) = create_test_replace_service();
    
    create_test_file(temp_dir.path(), "test.js", "console.log('a'); console.log('b');");
    
    let param = FileReplaceParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        replacement: "console.warn($VAR)".to_string(),
        language: "javascript".to_string(),
        dry_run: true,
        summary_only: true,
        ..Default::default()
    };

    let result = service.file_replace(param).await.unwrap();
    assert_eq!(result.total_changes, 2);
    
    // In summary mode, detailed diffs should be empty or limited
    if !result.files.is_empty() {
        assert!(result.files[0].changes.len() <= result.files[0].change_count);
    }
}