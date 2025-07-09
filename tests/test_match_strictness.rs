use ast_grep_mcp::{MatchStrictness, SearchParam};

#[test]
fn test_match_strictness_serialization() {
    // Test that MatchStrictness can be serialized/deserialized
    let strictness = MatchStrictness::Ast;
    let json = serde_json::to_string(&strictness).unwrap();
    assert_eq!(json, "\"ast\"");

    let deserialized: MatchStrictness = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, MatchStrictness::Ast);
}

#[test]
fn test_search_param_with_strictness() {
    let param = SearchParam {
        code: "console.log('test')".to_string(),
        pattern: "console.log($ARG)".to_string(),
        language: "javascript".to_string(),
        strictness: Some(MatchStrictness::Relaxed),
        selector: None,
        context: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let json = serde_json::to_string(&param).unwrap();
    assert!(json.contains("\"strictness\":\"relaxed\""));

    // Test that strictness is optional
    let param_without = SearchParam {
        code: "console.log('test')".to_string(),
        pattern: "console.log($ARG)".to_string(),
        language: "javascript".to_string(),
        strictness: None,
        selector: None,
        context: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let json = serde_json::to_string(&param_without).unwrap();
    assert!(!json.contains("strictness"));
}

#[test]
fn test_match_strictness_affects_results() {
    // This test will verify that different strictness levels produce different results
    // We'll implement this after the basic structure is in place
}
