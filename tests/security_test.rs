use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::{FileSearchParam, ast_grep_service::AstGrepService};
use std::fs;
use tempfile::TempDir;

fn create_secure_service() -> (AstGrepService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);
    (service, temp_dir)
}

#[tokio::test]
async fn test_path_traversal_blocked() {
    let (service, temp_dir) = create_secure_service();

    // Create a test file in the allowed directory
    fs::write(
        temp_dir.path().join("allowed.js"),
        "console.log('allowed');",
    )
    .unwrap();

    // Try various path traversal attempts
    let traversal_patterns = vec![
        "../../../etc/passwd",
        "../../..",
        "../escape.js",
        "/etc/passwd",
        "foo/../../../etc/passwd",
    ];

    for pattern in traversal_patterns {
        let param = FileSearchParam {
            path_pattern: pattern.to_string(),
            pattern: "console.log".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.file_search(param).await;

        // Path traversal patterns with ../ should error
        // Absolute paths outside roots should return no results
        match pattern {
            pat if pat.contains("..") => {
                assert!(
                    result.is_err(),
                    "Path traversal pattern '{pattern}' should be blocked"
                );
            }
            _ => {
                // Absolute paths outside roots should return no results
                // Absolute paths outside roots should return no results or error
                if let Ok(res) = result {
                    assert_eq!(
                        res.matches.len(),
                        0,
                        "Should not find files outside root for pattern '{pattern}'"
                    );
                }
                // Err(_) is also acceptable
            }
        }
    }
}

#[tokio::test]
async fn test_absolute_path_outside_root_blocked() {
    let (service, temp_dir) = create_secure_service();

    // Create a test file in the allowed directory
    fs::write(
        temp_dir.path().join("allowed.js"),
        "console.log('allowed');",
    )
    .unwrap();

    // Create a file outside the root (in system temp)
    let outside_file = std::env::temp_dir().join("outside_test.js");
    fs::write(&outside_file, "console.log('outside');").unwrap();

    // Try to access the file outside root with absolute path
    let param = FileSearchParam {
        path_pattern: outside_file.to_str().unwrap().to_string(),
        pattern: "console.log".to_string(),
        language: "javascript".to_string(),
        max_results: 10,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.file_search(param).await;

    // Should either error or return no results
    if let Ok(res) = result {
        assert_eq!(res.matches.len(), 0, "Should not find files outside root");
    }
    // Err(_) is also acceptable

    // Clean up
    let _ = fs::remove_file(&outside_file);
}

#[tokio::test]
async fn test_valid_patterns_still_work() {
    let (service, temp_dir) = create_secure_service();

    // Create test files
    fs::write(temp_dir.path().join("test.js"), "console.log('test');").unwrap();
    fs::create_dir(temp_dir.path().join("src")).unwrap();
    fs::write(temp_dir.path().join("src/main.js"), "console.log('main');").unwrap();

    // Test valid patterns
    let valid_patterns = vec!["**/*.js", "*.js", "src/*.js", "test.js"];

    for pattern in valid_patterns {
        let param = FileSearchParam {
            path_pattern: pattern.to_string(),
            pattern: "console.log".to_string(),
            language: "javascript".to_string(),
            max_results: 10,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.file_search(param).await.unwrap();
        assert!(
            !result.matches.is_empty(),
            "Valid pattern '{pattern}' should find matches"
        );
    }
}
