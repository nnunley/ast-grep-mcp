use ast_grep_mcp::ast_grep_service::{AstGrepService, SearchParam, FileSearchParam, ReplaceParam, FileReplaceParam, DocumentationParam};
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_file_search_integration() {
    let service = AstGrepService::new();
    
    // Create a temporary directory with test files
    let temp_dir = TempDir::new().unwrap();
    let js_file_path = temp_dir.path().join("test.js");
    let rust_file_path = temp_dir.path().join("test.rs");
    
    // Write test JavaScript file
    fs::write(&js_file_path, r#"
function greet() {
    console.log("Hello, world!");
    console.log("Welcome!");
}

function goodbye() {
    alert("Goodbye!");
}
"#).unwrap();
    
    // Write test Rust file
    fs::write(&rust_file_path, r#"
fn main() {
    println!("Hello, Rust!");
    println!("Testing ast-grep");
}
"#).unwrap();
    
    // Change to temp directory for file search
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    // Test file search for JavaScript
    let param = FileSearchParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.file_results.len(), 1);
    assert_eq!(result.file_results[0].matches.len(), 2);
    
    // Test file search for Rust
    let param = FileSearchParam {
        path_pattern: "*.rs".to_string(),
        pattern: "println!($VAR)".to_string(),
        language: "rust".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.file_results.len(), 1);
    assert_eq!(result.file_results[0].matches.len(), 2);
    
    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_file_replace_integration() {
    let service = AstGrepService::new();
    
    // Create a temporary directory with test files
    let temp_dir = TempDir::new().unwrap();
    let js_file_path = temp_dir.path().join("test.js");
    
    // Write test JavaScript file
    fs::write(&js_file_path, r#"
const x = 5;
const y = 10;
const z = 15;
"#).unwrap();
    
    // Change to temp directory for file replace
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    // Test file replace
    let param = FileReplaceParam {
        path_pattern: "*.js".to_string(),
        pattern: "const $VAR = $VAL".to_string(),
        replacement: "let $VAR = $VAL".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
    
    let result = service.file_replace(param).await.unwrap();
    assert_eq!(result.file_results.len(), 1);
    
    let rewritten_content = &result.file_results[0].rewritten_content;
    assert!(rewritten_content.contains("let x = 5"));
    assert!(rewritten_content.contains("let y = 10"));
    assert!(rewritten_content.contains("let z = 15"));
    assert!(!rewritten_content.contains("const"));
    
    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_glob_pattern_matching() {
    let service = AstGrepService::new();
    
    // Create a temporary directory with nested structure
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    
    let main_js = src_dir.join("main.js");
    let utils_js = src_dir.join("utils.js");
    let readme_md = temp_dir.path().join("README.md");
    
    // Write test files
    fs::write(&main_js, "console.log('main');").unwrap();
    fs::write(&utils_js, "console.log('utils');").unwrap();
    fs::write(&readme_md, "# README").unwrap();
    
    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    // Test specific glob pattern
    let param = FileSearchParam {
        path_pattern: "src/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.file_results.len(), 2); // Should find both JS files
    
    // Test wildcard pattern
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.file_results.len(), 2); // Should find both JS files
    
    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_error_handling_invalid_glob() {
    let service = AstGrepService::new();
    
    let param = FileSearchParam {
        path_pattern: "[invalid glob pattern".to_string(), // Invalid glob
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_size_limit() {
    let service = AstGrepService::new();
    
    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("large.js");
    
    // Create a large file (> 10MB would be skipped, but this is just a small test)
    let large_content = "console.log('test');\n".repeat(1000);
    fs::write(&large_file, large_content).unwrap();
    
    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    let param = FileSearchParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    // Should still process the file since it's under the limit
    assert_eq!(result.file_results.len(), 1);
    assert_eq!(result.file_results[0].matches.len(), 1000);
    
    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_multiple_languages_integration() {
    let service = AstGrepService::new();
    
    // Create a temporary directory with files in different languages
    let temp_dir = TempDir::new().unwrap();
    
    // JavaScript file
    let js_file = temp_dir.path().join("test.js");
    fs::write(&js_file, r#"
function add(a, b) {
    return a + b;
}
"#).unwrap();
    
    // Rust file
    let rs_file = temp_dir.path().join("test.rs");
    fs::write(&rs_file, r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#).unwrap();
    
    // Python file
    let py_file = temp_dir.path().join("test.py");
    fs::write(&py_file, r#"
def add(a, b):
    return a + b
"#).unwrap();
    
    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    // Test JavaScript function search
    let param = FileSearchParam {
        path_pattern: "*.js".to_string(),
        pattern: "function $NAME($PARAMS) { $BODY }".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.file_results.len(), 1);
    assert_eq!(result.file_results[0].matches.len(), 1);
    
    // Test Rust function search
    let param = FileSearchParam {
        path_pattern: "*.rs".to_string(),
        pattern: "fn $NAME($PARAMS) -> $RET { $BODY }".to_string(),
        language: "rust".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.file_results.len(), 1);
    assert_eq!(result.file_results[0].matches.len(), 1);
    
    // Test Python function search
    let param = FileSearchParam {
        path_pattern: "*.py".to_string(),
        pattern: "def $NAME($PARAMS): $BODY".to_string(),
        language: "python".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.file_results.len(), 1);
    assert_eq!(result.file_results[0].matches.len(), 1);
    
    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_no_matches_file_search() {
    let service = AstGrepService::new();
    
    // Create a temporary directory with test file
    let temp_dir = TempDir::new().unwrap();
    let js_file = temp_dir.path().join("test.js");
    
    fs::write(&js_file, r#"
function greet() {
    alert("Hello!");
}
"#).unwrap();
    
    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();
    
    // Search for pattern that doesn't exist
    let param = FileSearchParam {
        path_pattern: "*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };
    
    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.file_results.len(), 0); // No matches found
    
    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}