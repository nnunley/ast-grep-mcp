use ast_grep_mcp::{FileReplaceParam, FileSearchParam};

#[test]
fn test_file_search_param_with_struct_update_syntax() {
    // Test using ..Default::default() with new fields
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    // Should have default values for all fields
    assert_eq!(param.max_results, 20);
    assert_eq!(param.max_file_size, 50 * 1024 * 1024);
    assert!(param.cursor.is_none());
    assert!(param.strictness.is_none());
    assert!(param.selector.is_none());
    assert!(param.context.is_none());
}

#[test]
fn test_file_replace_param_with_struct_update_syntax() {
    // Test using ..Default::default() with new fields
    let param = FileReplaceParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "var $VAR = $VALUE".to_string(),
        replacement: "let $VAR = $VALUE".to_string(),
        language: "javascript".to_string(),
        ..Default::default()
    };

    // Should have default values for all fields
    assert_eq!(param.max_results, 10000);
    assert_eq!(param.max_file_size, 50 * 1024 * 1024);
    assert!(param.dry_run);
    assert!(!param.summary_only);
    assert!(!param.include_samples);
    assert_eq!(param.max_samples, 3);
    assert!(param.cursor.is_none());
    assert!(param.strictness.is_none());
    assert!(param.selector.is_none());
    assert!(param.context.is_none());
}

#[test]
fn test_cloning_params_with_new_fields() {
    let param1 = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        selector: Some("call_expression".to_string()),
        context: Some("function() { $PATTERN }".to_string()),
        ..Default::default()
    };

    // Test cloning and then modifying with spread
    let param2 = FileSearchParam {
        max_results: 5,
        ..param1.clone()
    };

    assert_eq!(param2.max_results, 5);
    assert_eq!(param2.selector, Some("call_expression".to_string()));
    assert_eq!(param2.context, Some("function() { $PATTERN }".to_string()));
}
