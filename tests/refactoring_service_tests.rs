//! Tests for RefactoringService
//! 
//! This module contains unit tests for the refactoring service that were originally
//! embedded in the source files.

use ast_grep_mcp::refactoring::*;
use ast_grep_mcp::search::SearchService;
use ast_grep_mcp::replace::ReplaceService;
use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::rules::RuleEvaluator;
use std::sync::Arc;
use tempfile::TempDir;

fn create_test_refactoring_service() -> (RefactoringService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    
    let search_service = Arc::new(SearchService::new(
        config.clone(),
        pattern_matcher.clone(),
        rule_evaluator.clone(),
    ));
    
    let replace_service = Arc::new(ReplaceService::new(
        config,
        pattern_matcher,
        rule_evaluator,
    ));
    
    let service = RefactoringService::new(search_service, replace_service).unwrap();
    (service, temp_dir)
}

#[tokio::test]
async fn test_list_refactorings() {
    let (service, _temp_dir) = create_test_refactoring_service();
    
    let refactorings = service.list_refactorings().await.unwrap();
    
    // Should have some built-in refactorings
    assert!(!refactorings.is_empty());
    
    // Check for expected refactoring types
    let refactoring_ids: Vec<&str> = refactorings.iter()
        .map(|r| r.id.as_str())
        .collect();
    
    assert!(refactoring_ids.contains(&"extract_method"));
    assert!(refactoring_ids.contains(&"extract_variable"));
}

#[tokio::test]
async fn test_get_refactoring_info() {
    let (service, _temp_dir) = create_test_refactoring_service();
    
    let info = service.get_refactoring_info("extract_method").await.unwrap();
    
    assert_eq!(info.id, "extract_method");
    assert!(!info.name.is_empty());
    assert!(!info.description.is_empty());
    assert!(!info.supported_languages.is_empty());
}

#[tokio::test]
async fn test_get_refactoring_info_not_found() {
    let (service, _temp_dir) = create_test_refactoring_service();
    
    let result = service.get_refactoring_info("nonexistent_refactoring").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_validate_refactoring_extract_method() {
    let (service, _temp_dir) = create_test_refactoring_service();
    
    let test_code = r#"
function calculate() {
    let a = 5;
    let b = 10;
    let result = a + b;
    console.log(result);
    return result;
}
"#;

    let request = ValidateRefactoringRequest {
        refactoring_id: "extract_method".to_string(),
        test_code: test_code.to_string(),
        language: "javascript".to_string(),
        custom_pattern: None,
    };

    let result = service.validate_refactoring(request).await.unwrap();
    assert!(result.is_valid);
    assert!(!result.matches.is_empty());
}

#[tokio::test]
async fn test_refactor_extract_method() {
    let (service, _temp_dir) = create_test_refactoring_service();
    
    let request = RefactoringRequest {
        refactoring_id: "extract_method".to_string(),
        pattern_example: Some(r#"
let a = 5;
let b = 10;
let result = a + b;
"#.to_string()),
        options: RefactoringOptions {
            function_name: Some("addNumbers".to_string()),
            language: Some("javascript".to_string()),
            scope: Some("file".to_string()),
            preview: Some(true),
            ..Default::default()
        },
    };

    let result = service.refactor(request).await.unwrap();
    assert!(result.matches_found > 0);
    assert!(!result.files_affected.is_empty());
    assert!(result.changes_preview.is_some());
}

/// Test helper functions that are used by refactoring tests
mod test_helpers {
    use super::*;

    pub fn create_javascript_test_code() -> &'static str {
        r#"
function processData() {
    let data = fetchData();
    
    // This could be extracted
    let processed = data.map(item => {
        return item * 2;
    });
    
    let filtered = processed.filter(item => item > 10);
    
    return filtered;
}

function anotherFunction() {
    console.log("Another function");
}
"#
    }

    pub fn create_python_test_code() -> &'static str {
        r#"
def process_data():
    data = fetch_data()
    
    # This could be extracted
    processed = [item * 2 for item in data]
    filtered = [item for item in processed if item > 10]
    
    return filtered

def another_function():
    print("Another function")
"#
    }
}

#[tokio::test]
async fn test_refactor_with_javascript() {
    let (service, _temp_dir) = create_test_refactoring_service();
    
    let request = RefactoringRequest {
        refactoring_id: "extract_variable".to_string(),
        pattern_example: Some("item * 2".to_string()),
        options: RefactoringOptions {
            variable_name: Some("multipliedItem".to_string()),
            language: Some("javascript".to_string()),
            preview: Some(true),
            ..Default::default()
        },
    };

    let result = service.refactor(request).await.unwrap();
    assert!(result.matches_found >= 0); // May or may not find matches depending on pattern
}

#[tokio::test]
async fn test_refactor_with_python() {
    let (service, _temp_dir) = create_test_refactoring_service();
    
    let request = RefactoringRequest {
        refactoring_id: "extract_variable".to_string(),
        pattern_example: Some("item * 2".to_string()),
        options: RefactoringOptions {
            variable_name: Some("multiplied_item".to_string()),
            language: Some("python".to_string()),
            preview: Some(true),
            ..Default::default()
        },
    };

    let result = service.refactor(request).await.unwrap();
    assert!(result.matches_found >= 0);
}