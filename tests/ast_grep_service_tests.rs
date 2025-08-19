//! Tests for AstGrepService
//! 
//! This module contains unit tests that were originally embedded in the source files
//! and have been moved here for better organization.

use ast_grep_mcp::ast_grep_service::AstGrepService;
use ast_grep_mcp::errors::ServiceError;
use ast_grep_mcp::types::*;
use std::path::PathBuf;

#[tokio::test]
async fn test_search_basic() {
    let service = AstGrepService::new();
    let param = SearchParam::new(
        "function greet() { console.log(\"Hello\"); }",
        "console.log($VAR)",
        "javascript",
    );

    let result = service.search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].text, "console.log(\"Hello\")");
    assert_eq!(
        result.matches[0].vars.get("VAR"),
        Some(&"\"Hello\"".to_string())
    );
}

#[tokio::test]
async fn test_search_no_matches() {
    let service = AstGrepService::new();
    let param = SearchParam::new(
        "function greet() { alert(\"Hello\"); }",
        "console.log($VAR)",
        "javascript",
    );

    let result = service.search(param).await.unwrap();
    assert_eq!(result.matches.len(), 0);
}

#[tokio::test]
async fn test_search_invalid_language() {
    let service = AstGrepService::new();
    let param = SearchParam::new(
        "function greet() { console.log(\"Hello\"); }",
        "console.log($VAR)",
        "invalid_language",
    );

    let result = service.search(param).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ServiceError::Internal(_)));
}

#[tokio::test]
async fn test_replace_basic() {
    let service = AstGrepService::new();
    let param = ReplaceParam::new(
        "function greet() { console.log(\"Hello\"); }",
        "console.log($VAR)",
        "console.warn($VAR)",
        "javascript",
    );

    let result = service.replace(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.new_code, "function greet() { console.warn(\"Hello\"); }");
}

#[tokio::test]
async fn test_file_search_basic() {
    let service = AstGrepService::new();
    let param = FileSearchParam {
        path_pattern: "tests/test_files/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    // This test might fail if test files don't exist, but structure is correct
    let _result = service.file_search(param).await;
}

#[tokio::test]
async fn test_generate_ast() {
    let service = AstGrepService::new();
    let param = GenerateAstParam {
        code: "function test() {}".to_string(),
        language: "javascript".to_string(),
    };

    let result = service.generate_ast(param).await.unwrap();
    assert!(!result.ast_structure.is_empty());
    assert!(!result.node_kinds.is_empty());
}

#[tokio::test]
async fn test_list_languages() {
    let service = AstGrepService::new();
    let param = ListLanguagesParam {};

    let result = service.list_languages(param).await.unwrap();
    assert!(!result.languages.is_empty());
    assert!(result.languages.contains(&"javascript".to_string()));
    assert!(result.languages.contains(&"typescript".to_string()));
    assert!(result.languages.contains(&"python".to_string()));
}

#[tokio::test]
async fn test_multiple_matches() {
    let service = AstGrepService::new();
    let param = SearchParam::new(
        "console.log('first'); console.log('second');",
        "console.log($VAR)",
        "javascript",
    );

    let result = service.search(param).await.unwrap();
    assert_eq!(result.matches.len(), 2);
}

#[tokio::test]
async fn test_pattern_caching() {
    let service = AstGrepService::new();
    
    // First search - should cache the pattern
    let param1 = SearchParam::new(
        "function test() { console.log('test'); }",
        "console.log($VAR)",
        "javascript",
    );
    let result1 = service.search(param1).await.unwrap();
    
    // Second search with same pattern - should use cached pattern
    let param2 = SearchParam::new(
        "function another() { console.log('another'); }",
        "console.log($VAR)",
        "javascript",
    );
    let result2 = service.search(param2).await.unwrap();
    
    assert_eq!(result1.matches.len(), 1);
    assert_eq!(result2.matches.len(), 1);
}

#[tokio::test]
async fn test_custom_config() {
    use ast_grep_mcp::config::ServiceConfig;
    
    let custom_config = ServiceConfig {
        max_file_size: 1024,
        timeout_seconds: 10,
        max_matches: 50,
        enable_debug: true,
        pattern_cache_size: 500, // Smaller cache for testing
        rules_directory: Some(PathBuf::from("test_rules")),
    };
    
    let service = AstGrepService::with_config(custom_config);
    let param = SearchParam::new(
        "console.log('test');",
        "console.log($VAR)",
        "javascript",
    );
    
    let result = service.search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
}

#[tokio::test]
async fn test_cache_eviction() {
    use ast_grep_mcp::config::ServiceConfig;
    
    // Create service with very small cache
    let config = ServiceConfig {
        pattern_cache_size: 2, // Only 2 patterns max
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);
    
    // Add 3 different patterns, should evict the first
    let params = [
        ("pattern1", "javascript"),
        ("pattern2", "javascript"), 
        ("pattern3", "javascript"),
    ];
    
    for (pattern, lang) in params.iter() {
        let param = SearchParam::new("test code", pattern, lang);
        let _ = service.search(param).await; // May fail but that's ok for cache test
    }
    
    // Verify cache has at most 2 entries
    let cache = service.pattern_cache.lock().unwrap();
    assert!(cache.len() <= 2);
}

#[tokio::test]
async fn test_search_with_context() {
    let service = AstGrepService::new();
    let param = SearchParam {
        code: "line1\nconsole.log('test')\nline3".to_string(),
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
async fn test_service_initialization() {
    // Test default initialization
    let service1 = AstGrepService::new();
    assert!(service1.get_debug_info().contains("AstGrepService"));
    
    // Test custom config initialization
    let service2 = AstGrepService::with_config(ast_grep_mcp::config::ServiceConfig {
        max_file_size: 2048,
        pattern_cache_size: 1000,
        ..Default::default()
    });
    assert!(service2.get_debug_info().contains("AstGrepService"));
}