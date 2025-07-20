//! Test for the learning system integration

use ast_grep_mcp::ast_grep_service::AstGrepService;
use ast_grep_mcp::learning::{ExplorePatternParam, ValidatePatternParam};

#[tokio::test]
async fn test_validate_pattern_basic() {
    let service = AstGrepService::new();

    let param = ValidatePatternParam {
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        test_code: Some("console.log('hello world');".to_string()),
        context: None,
    };

    let result = service.validate_pattern(param).await.unwrap();

    assert!(result.is_valid);
    assert!(result.match_result.is_some());
    assert!(result.analysis.complexity_score > 0.0);
    assert!(!result.analysis.language_compatibility.is_empty());
    assert!(!result.learning_insights.is_empty());
    assert!(!result.suggested_experiments.is_empty());
}

#[tokio::test]
async fn test_validate_pattern_invalid() {
    let service = AstGrepService::new();

    let param = ValidatePatternParam {
        pattern: "console.log($VAR)".to_string(),
        language: "javascript".to_string(),
        test_code: Some("alert('hello');".to_string()), // Doesn't match pattern
        context: None,
    };

    let result = service.validate_pattern(param).await.unwrap();

    assert!(!result.is_valid); // Should be false because pattern doesn't match test code
    assert!(result.match_result.is_none()); // No matches found
    assert!(!result.learning_insights.is_empty()); // Should still provide insights
}

#[tokio::test]
async fn test_explore_patterns_all() {
    let service = AstGrepService::new();

    let param = ExplorePatternParam {
        language: None,
        category: None,
        complexity: None,
        search: None,
        limit: Some(10),
    };

    let result = service.explore_patterns(param).await.unwrap();

    assert!(!result.patterns.is_empty());
    assert!(result.total_available > 0);
    assert!(!result.learning_path.is_empty());
}

#[tokio::test]
async fn test_explore_patterns_by_language() {
    let service = AstGrepService::new();

    let param = ExplorePatternParam {
        language: Some("javascript".to_string()),
        category: None,
        complexity: None,
        search: None,
        limit: Some(5),
    };

    let result = service.explore_patterns(param).await.unwrap();

    // All returned patterns should be for JavaScript
    for pattern in &result.patterns {
        assert_eq!(pattern.language, "javascript");
    }
    assert!(!result.learning_path.is_empty());
}

#[tokio::test]
async fn test_explore_patterns_by_complexity() {
    let service = AstGrepService::new();

    let param = ExplorePatternParam {
        language: None,
        category: None,
        complexity: Some("beginner".to_string()),
        search: None,
        limit: Some(5),
    };

    let result = service.explore_patterns(param).await.unwrap();

    // All returned patterns should be beginner level
    for pattern in &result.patterns {
        assert_eq!(pattern.difficulty, "beginner");
    }
}

#[tokio::test]
async fn test_explore_patterns_search() {
    let service = AstGrepService::new();

    let param = ExplorePatternParam {
        language: None,
        category: None,
        complexity: None,
        search: Some("function".to_string()),
        limit: Some(10),
    };

    let result = service.explore_patterns(param).await.unwrap();

    // Results should contain patterns related to functions
    let has_function_related = result.patterns.iter().any(|p| {
        p.pattern.contains("function")
            || p.description.to_lowercase().contains("function")
            || p.tags
                .iter()
                .any(|tag| tag.to_lowercase().contains("function"))
    });

    assert!(has_function_related);
}
