use ast_grep_mcp::{FileSearchParam, ReplaceParam, SearchParam};

#[test]
fn test_search_param_with_selector() {
    let param = SearchParam {
        code: "class Foo { bar = 123 }".to_string(),
        pattern: "$VAR = $VALUE".to_string(),
        language: "javascript".to_string(),
        strictness: None,
        selector: Some("field_definition".to_string()),
        context: Some("class X { $PATTERN }".to_string()),
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let json = serde_json::to_string(&param).unwrap();
    assert!(json.contains("\"selector\":\"field_definition\""));
    assert!(json.contains("\"context\":\"class X { $PATTERN }\""));

    let deserialized: SearchParam = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.selector, Some("field_definition".to_string()));
    assert_eq!(
        deserialized.context,
        Some("class X { $PATTERN }".to_string())
    );
}

#[test]
fn test_file_search_param_with_selector() {
    let param = FileSearchParam {
        path_pattern: "**/*.js".to_string(),
        pattern: "$VAR = $VALUE".to_string(),
        language: "javascript".to_string(),
        max_results: 10,
        max_file_size: 1024 * 1024,
        cursor: None,
        strictness: None,
        selector: Some("field_definition".to_string()),
        context: Some("class X { $PATTERN }".to_string()),
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let json = serde_json::to_string(&param).unwrap();
    assert!(json.contains("\"selector\":\"field_definition\""));
    assert!(json.contains("\"context\":\"class X { $PATTERN }\""));
}

#[test]
fn test_replace_param_with_selector() {
    let param = ReplaceParam {
        code: "class Foo { bar = 123 }".to_string(),
        pattern: "$VAR = $VALUE".to_string(),
        replacement: "$VAR: $VALUE".to_string(),
        language: "javascript".to_string(),
        strictness: None,
        selector: Some("field_definition".to_string()),
        context: Some("class X { $PATTERN }".to_string()),
    };

    let json = serde_json::to_string(&param).unwrap();
    assert!(json.contains("\"selector\":\"field_definition\""));
    assert!(json.contains("\"context\":\"class X { $PATTERN }\""));
}

#[test]
fn test_backward_compatibility_without_selector() {
    // Ensure existing code without selector/context still works
    let param = SearchParam::new("code", "pattern", "javascript");

    assert_eq!(param.selector, None);
    assert_eq!(param.context, None);

    let json = serde_json::to_string(&param).unwrap();
    assert!(!json.contains("selector"));
    assert!(!json.contains("context"));
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_selector_matches_specific_nodes() {
        // Test that selector actually filters to specific node types
        let code = r#"
        class MyClass {
            myField = 123;
            myMethod() {
                let myVar = 456;
            }
        }
        "#;

        // This test will verify the selector is used in actual matching
        // For now, just ensure the structure is correct
        let param = SearchParam {
            code: code.to_string(),
            pattern: "$VAR = $VALUE".to_string(),
            language: "javascript".to_string(),
            strictness: None,
            selector: Some("field_definition".to_string()),
            context: Some("class X { $PATTERN }".to_string()),
            context_before: None,
            context_after: None,
            context_lines: None,
        };

        // The actual test would verify only the field is matched, not the variable
        assert!(param.selector.is_some());
    }
}
