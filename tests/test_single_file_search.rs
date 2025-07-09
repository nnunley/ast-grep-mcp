use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::rules::RuleEvaluator;
use ast_grep_mcp::search::SearchService;
use ast_grep_mcp::types::FileSearchParam;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_single_file_search_with_matches() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    let test_file = temp_path.join("test.rs");

    // Write test content with #[test] attribute
    let test_content = r#"
#[test]
fn test_something() {
    assert_eq!(1, 1);
}

fn regular_function() {
    println!("Hello");
}

#[test]
fn another_test() {
    assert!(true);
}
"#;

    fs::write(&test_file, test_content).unwrap();

    // Create service with temp directory as root
    let config = ServiceConfig {
        root_directories: vec![temp_path.to_path_buf()],
        max_file_size: 1024 * 1024,
        max_concurrency: 10,
        limit: 100,
        rules_directory: PathBuf::from(".ast-grep-rules"),
        pattern_cache_size: 1000,
    };

    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let service = SearchService::new(config, pattern_matcher, rule_evaluator);

    // Test searching for #[test] pattern in the specific file
    let param = FileSearchParam {
        path_pattern: test_file.to_string_lossy().to_string(),
        pattern: "#[test]".to_string(),
        language: "rust".to_string(),
        max_results: 10,
        cursor: None,
        max_file_size: 1024 * 1024,
    };

    let result = service.file_search(param).await.unwrap();

    // Should find matches
    assert_eq!(result.total_files_found, 1);
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].file_path, test_file.to_string_lossy());
    assert_eq!(result.matches[0].matches.len(), 2); // Two #[test] attributes

    // Cursor should indicate completion
    assert!(result.next_cursor.is_some());
    assert!(result.next_cursor.unwrap().is_complete);
}

#[tokio::test]
async fn test_single_file_search_no_matches() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    let test_file = temp_path.join("test.rs");

    // Write test content without #[test] attribute
    let test_content = r#"
fn regular_function() {
    println!("Hello");
}

fn another_function() {
    println!("World");
}
"#;

    fs::write(&test_file, test_content).unwrap();

    // Create service with temp directory as root
    let config = ServiceConfig {
        root_directories: vec![temp_path.to_path_buf()],
        max_file_size: 1024 * 1024,
        max_concurrency: 10,
        limit: 100,
        rules_directory: PathBuf::from(".ast-grep-rules"),
        pattern_cache_size: 1000,
    };

    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let service = SearchService::new(config, pattern_matcher, rule_evaluator);

    // Test searching for #[test] pattern in the specific file
    let param = FileSearchParam {
        path_pattern: test_file.to_string_lossy().to_string(),
        pattern: "#[test]".to_string(),
        language: "rust".to_string(),
        max_results: 10,
        cursor: None,
        max_file_size: 1024 * 1024,
    };

    let result = service.file_search(param).await.unwrap();

    // Should find no matches
    assert_eq!(result.total_files_found, 0);
    assert_eq!(result.matches.len(), 0);

    // Cursor should indicate completion
    assert!(result.next_cursor.is_some());
    assert!(result.next_cursor.unwrap().is_complete);
}

#[tokio::test]
async fn test_single_file_outside_root_directory() {
    // Create a temporary directory and file
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create another temp dir outside the root
    let other_temp_dir = TempDir::new().unwrap();
    let other_temp_path = other_temp_dir.path();
    let test_file = other_temp_path.join("test.rs");

    fs::write(&test_file, "fn test() {}").unwrap();

    // Create service with only the first temp directory as root
    let config = ServiceConfig {
        root_directories: vec![temp_path.to_path_buf()],
        max_file_size: 1024 * 1024,
        max_concurrency: 10,
        limit: 100,
        rules_directory: PathBuf::from(".ast-grep-rules"),
        pattern_cache_size: 1000,
    };

    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();
    let service = SearchService::new(config, pattern_matcher, rule_evaluator);

    // Test searching for pattern in file outside root directory
    let param = FileSearchParam {
        path_pattern: test_file.to_string_lossy().to_string(),
        pattern: "fn".to_string(),
        language: "rust".to_string(),
        max_results: 10,
        cursor: None,
        max_file_size: 1024 * 1024,
    };

    let result = service.file_search(param).await;

    // Should fail with security error
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not under any configured root directory")
    );
}
