use std::fs;

use ast_grep_mcp::ast_grep_service::AstGrepService;
use ast_grep_mcp::{FileReplaceParam, FileSearchParam};
use ast_grep_mcp::config::ServiceConfig;
use tempfile::TempDir;

#[tokio::test]
async fn test_file_search_integration() {
    // Create a temporary directory with test files
    let temp_dir = TempDir::new().unwrap();

    // Create service with specific root directory
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);
    let js_file_path = temp_dir.path().join("test.js");
    let rust_file_path = temp_dir.path().join("test.rs");

    // Write test JavaScript file
    fs::write(
        &js_file_path,
        r#"
function greet() {
    console.log("Hello, world!");
    console.log("Welcome!");
}

function goodbye() {
    alert("Goodbye!");
}
"#,
    )
    .unwrap();

    // Write test Rust file
    fs::write(
        &rust_file_path,
        r#"
fn main() {
    println!("Hello, Rust!");
    println!("Testing ast-grep");
}
"#,
    )
    .unwrap();

    // Test file search for JavaScript using glob pattern
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 2);

    // Test file search for Rust using glob pattern
    let param = FileSearchParam {
        path_pattern: "**/*.rs".to_string(),
        pattern: "println!($VAR)".to_string(),
        language: "rust".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 2);
}

#[tokio::test]
async fn test_file_replace_integration() {
    // Create a temporary directory with test files
    let temp_dir = TempDir::new().unwrap();

    // Create service with specific root directory
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let js_file_path = temp_dir.path().join("test.js");

    // Write test JavaScript file
    fs::write(
        &js_file_path,
        r#"const x = 5;
const y = 10;
const z = 15;"#,
    )
    .unwrap();

    // Test file replace using glob path pattern
    let param = FileReplaceParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "const $VAR = $VAL".to_string(),
        replacement: "let $VAR = $VAL".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_replace(param).await.unwrap();
    assert_eq!(result.file_results.len(), 1);
    assert!(result.dry_run); // Should be true by default

    let file_result = &result.file_results[0];
    assert_eq!(file_result.total_changes, 3); // Should have 3 changes (x, y, z)

    // Check that the changes are as expected
    let changes = &file_result.changes;
    assert_eq!(changes.len(), 3);

    // Verify each change converts const to let
    for change in changes {
        assert!(change.old_text.starts_with("const"));
        assert!(change.new_text.starts_with("let"));
    }
}

#[tokio::test]
async fn test_glob_pattern_matching() {
    // Create a temporary directory with nested structure
    let temp_dir = TempDir::new().unwrap();

    // Create service with specific root directory
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();

    let main_js = src_dir.join("main.js");
    let utils_js = src_dir.join("utils.js");
    let readme_md = temp_dir.path().join("README.md");

    // Write test files
    fs::write(&main_js, "console.log('main');").unwrap();
    fs::write(&utils_js, "console.log('utils');").unwrap();
    fs::write(&readme_md, "# README").unwrap();

    // Test recursive glob pattern
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 2); // Should find both JS files

    // Test wildcard pattern (same as above, should get same results)
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 2); // Should find both JS files
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
    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();

    // Create service with specific root directory
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let large_file = temp_dir.path().join("large.js");

    // Create a large file (> 10MB would be skipped, but this is just a small test)
    let large_content = "console.log('test');\n".repeat(1000);
    fs::write(&large_file, large_content).unwrap();

    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    // Should still process the file since it's under the limit
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 1000);
}

#[tokio::test]
async fn test_multiple_languages_integration() {
    // Create a temporary directory with files in different languages
    let temp_dir = TempDir::new().unwrap();

    // Create service with specific root directory
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // JavaScript file
    let js_file = temp_dir.path().join("test.js");
    fs::write(
        &js_file,
        r#"function add(a, b) {
    return a + b;
}"#,
    )
    .unwrap();

    // Rust file
    let rs_file = temp_dir.path().join("test.rs");
    fs::write(
        &rs_file,
        r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    )
    .unwrap();

    // Python file
    let py_file = temp_dir.path().join("test.py");
    fs::write(
        &py_file,
        r#"
def add(a, b):
    return a + b
"#,
    )
    .unwrap();

    // Test JavaScript function search
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "function $NAME($A, $B) { return $RET }".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 1);

    // Test Rust function search
    let param = FileSearchParam {
        path_pattern: "**/*.rs".to_string(),
        pattern: "fn $NAME($A: $TA, $B: $TB) -> $RET { $BODY }".to_string(),
        language: "rust".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 1);

    // Test Python function search
    let param = FileSearchParam {
        path_pattern: "**/*.py".to_string(),
        pattern: "def $NAME($A, $B): return $RET".to_string(),
        language: "python".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 1);
}

#[tokio::test]
async fn test_no_matches_file_search() {
    // Create a temporary directory with test file
    let temp_dir = TempDir::new().unwrap();

    // Create service with specific root directory
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let js_file = temp_dir.path().join("test.js");

    fs::write(
        &js_file,
        r#"
function greet() {
    alert("Hello!");
}
"#,
    )
    .unwrap();

    // Search for pattern that doesn't exist
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 0); // No matches found
}

#[tokio::test]
async fn test_replace_string_literal_to_into() {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let rust_file_path = temp_dir.path().join("test_replace.rs");
    fs::write(
        &rust_file_path,
        r#"
fn main() {
    let message = "Hello".to_string();
    let another_message = "World".to_string();
    println!("{}", message);
}
"#,
    )
    .unwrap();

    let param = FileReplaceParam {
        path_pattern: "**/*.rs".to_string(),
        pattern: "$VAR.to_string()".to_string(),
        replacement: "$VAR.into()".to_string(),
        language: "rust".to_string(),
        dry_run: false, // Apply the changes
        ..Default::default()
    };

    let result = service.file_replace(param).await.unwrap();
    assert_eq!(result.file_results.len(), 1);
    assert!(!result.dry_run);

    let modified_content = fs::read_to_string(&rust_file_path).unwrap();
    assert!(modified_content.contains("let message = \"Hello\".into();"));
    assert!(modified_content.contains("let another_message = \"World\".into();"));
    assert!(!modified_content.contains(".to_string()"));
}
