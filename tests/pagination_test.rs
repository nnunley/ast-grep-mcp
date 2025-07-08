use ast_grep_mcp::ast_grep_service::AstGrepService;
use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::{FileReplaceParam, FileSearchParam};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_file_search_pagination_basic() {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create 10 test files
    for i in 0..10 {
        let file_path = temp_dir.path().join(format!("test{:02}.js", i));
        fs::write(&file_path, format!("console.log('file {}');", i)).unwrap();
    }

    // Search with small page size
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        max_results: 3,
        ..Default::default()
    };

    let result1 = service.file_search(param.clone()).await.unwrap();
    println!(
        "Result1: {} matches, {} total_files_found",
        result1.matches.len(),
        result1.total_files_found
    );
    for m in &result1.matches {
        println!("  File: {}", m.file_path);
    }
    assert_eq!(result1.matches.len(), 3);
    // total_files_found counts files with matches, not all files
    assert_eq!(result1.total_files_found, 3);
    assert!(result1.next_cursor.is_some());
    assert!(!result1.next_cursor.as_ref().unwrap().is_complete);

    // Get next page
    let mut param2 = param.clone();
    param2.cursor = result1
        .next_cursor
        .clone()
        .map(|c| ast_grep_mcp::CursorParam {
            last_file_path: c.last_file_path,
            is_complete: c.is_complete,
        });
    let result2 = service.file_search(param2).await.unwrap();
    assert_eq!(result2.matches.len(), 3);
    assert!(!result2.next_cursor.as_ref().unwrap().is_complete);

    // Continue pagination
    let mut param3 = param.clone();
    param3.cursor = result2
        .next_cursor
        .clone()
        .map(|c| ast_grep_mcp::CursorParam {
            last_file_path: c.last_file_path,
            is_complete: c.is_complete,
        });
    let result3 = service.file_search(param3).await.unwrap();
    println!("Result3: {} matches", result3.matches.len());
    for m in &result3.matches {
        println!("  File: {}", m.file_path);
    }
    assert_eq!(result3.matches.len(), 3);
    assert!(!result3.next_cursor.as_ref().unwrap().is_complete);

    // Get final page
    let mut param4 = param.clone();
    param4.cursor = result3
        .next_cursor
        .clone()
        .map(|c| ast_grep_mcp::CursorParam {
            last_file_path: c.last_file_path,
            is_complete: c.is_complete,
        });
    let result4 = service.file_search(param4).await.unwrap();
    assert_eq!(result4.matches.len(), 1); // Only 1 file left
    assert!(result4.next_cursor.as_ref().unwrap().is_complete);

    // Verify no duplicate files across pages
    let mut all_files = vec![];
    all_files.extend(result1.matches.iter().map(|m| &m.file_path));
    all_files.extend(result2.matches.iter().map(|m| &m.file_path));
    all_files.extend(result3.matches.iter().map(|m| &m.file_path));
    all_files.extend(result4.matches.iter().map(|m| &m.file_path));
    all_files.sort();
    all_files.dedup();
    assert_eq!(all_files.len(), 10);
}

#[tokio::test]
async fn test_file_search_pagination_complete_cursor() {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create test file
    let file_path = temp_dir.path().join("test.js");
    fs::write(&file_path, "console.log('test');").unwrap();

    // Search with cursor marked as complete
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        cursor: Some(ast_grep_mcp::CursorParam {
            last_file_path: String::new(),
            is_complete: true,
        }),
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 0);
    assert_eq!(result.total_files_found, 0);
    assert!(result.next_cursor.as_ref().unwrap().is_complete);
}

#[tokio::test]
async fn test_file_replace_pagination() {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create 5 test files
    for i in 0..5 {
        let file_path = temp_dir.path().join(format!("test{}.js", i));
        fs::write(&file_path, "const x = 5; const y = 10;").unwrap();
    }

    // Replace with small page size
    let param = FileReplaceParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "const $VAR = $VAL".to_string(),
        replacement: "let $VAR = $VAL".to_string(),
        language: "javascript".to_string(),
        max_results: 2,
        dry_run: true,
        ..Default::default()
    };

    let result1 = service.file_replace(param.clone()).await.unwrap();
    assert_eq!(result1.file_results.len(), 2);
    // total_files_found counts files with changes, not all files
    assert_eq!(result1.total_files_found, 2);
    assert_eq!(result1.files_with_changes, 2);
    assert_eq!(result1.total_changes, 4); // 2 changes per file
    assert!(!result1.next_cursor.as_ref().unwrap().is_complete);

    // Get next page
    let mut param2 = param.clone();
    param2.cursor = result1
        .next_cursor
        .clone()
        .map(|c| ast_grep_mcp::CursorParam {
            last_file_path: c.last_file_path,
            is_complete: c.is_complete,
        });
    let result2 = service.file_replace(param2).await.unwrap();
    assert_eq!(result2.file_results.len(), 2);
    assert!(!result2.next_cursor.as_ref().unwrap().is_complete);

    // Get final page
    let mut param3 = param.clone();
    param3.cursor = result2
        .next_cursor
        .clone()
        .map(|c| ast_grep_mcp::CursorParam {
            last_file_path: c.last_file_path,
            is_complete: c.is_complete,
        });
    let result3 = service.file_replace(param3).await.unwrap();
    assert_eq!(result3.file_results.len(), 1);
    assert!(result3.next_cursor.as_ref().unwrap().is_complete);
}

#[tokio::test]
async fn test_pagination_with_no_matches() {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create files without matches
    for i in 0..5 {
        let file_path = temp_dir.path().join(format!("test{}.js", i));
        fs::write(&file_path, "alert('hello');").unwrap();
    }

    // Search for pattern that doesn't exist
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        max_results: 2,
        ..Default::default()
    };

    let result = service.file_search(param).await.unwrap();
    assert_eq!(result.matches.len(), 0);
    assert_eq!(result.total_files_found, 0);
    assert!(result.next_cursor.as_ref().unwrap().is_complete);
}

#[tokio::test]
async fn test_pagination_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create files with predictable names for consistent ordering
    for i in 0..20 {
        let file_path = temp_dir.path().join(format!("file_{:03}.js", i));
        fs::write(&file_path, format!("console.log('test {}');", i)).unwrap();
    }

    // Get all results with large page size
    let param_all = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        max_results: 100,
        ..Default::default()
    };

    let result_all = service.file_search(param_all).await.unwrap();
    assert_eq!(result_all.matches.len(), 20);
    assert_eq!(result_all.total_files_found, 20);

    // Get results with pagination
    let mut paginated_results = vec![];
    let mut cursor: Option<ast_grep_mcp::CursorParam> = None;
    let page_size = 5;

    loop {
        let param = FileSearchParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
            max_results: page_size,
            cursor: cursor.clone(),
            ..Default::default()
        };

        let result = service.file_search(param).await.unwrap();
        paginated_results.extend(result.matches.clone());

        if result.next_cursor.as_ref().unwrap().is_complete {
            break;
        }
        cursor = result.next_cursor.map(|c| ast_grep_mcp::CursorParam {
            last_file_path: c.last_file_path,
            is_complete: c.is_complete,
        });
    }

    // Verify same results
    assert_eq!(paginated_results.len(), result_all.matches.len());

    // Verify order is consistent
    for (i, (paginated, all)) in paginated_results
        .iter()
        .zip(result_all.matches.iter())
        .enumerate()
    {
        assert_eq!(
            paginated.file_path, all.file_path,
            "Mismatch at position {}",
            i
        );
    }
}
