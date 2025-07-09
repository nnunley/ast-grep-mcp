use ast_grep_mcp::ast_grep_service::AstGrepService;
use ast_grep_mcp::types::{SpecificityLevel, SuggestPatternsParam};

#[tokio::test]
async fn test_suggest_patterns_tool_registered() {
    // For now, just test that we can create a service and call the method
    // The actual tool registration test will be added later when we figure out the MCP test setup
    let service = AstGrepService::new();
    let params = SuggestPatternsParam {
        code_examples: vec!["function test() {}".to_string()],
        language: "javascript".to_string(),
        max_suggestions: Some(1),
        specificity_levels: None,
    };
    let result = service.suggest_patterns(params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_suggest_patterns_accepts_basic_params() {
    let params = SuggestPatternsParam {
        code_examples: vec!["function test() {}".to_string()],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_exact_pattern_generation() {
    let params = SuggestPatternsParam {
        code_examples: vec!["console.log('hello')".to_string()],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await.unwrap();

    assert_eq!(result.suggestions.len(), 1);
    assert_eq!(result.suggestions[0].pattern, "console.log('hello')");
    assert_eq!(result.suggestions[0].specificity, SpecificityLevel::Exact);
}

#[tokio::test]
async fn test_multiple_identical_examples() {
    let params = SuggestPatternsParam {
        code_examples: vec![
            "console.log('test')".to_string(),
            "console.log('test')".to_string(),
        ],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await.unwrap();

    assert_eq!(result.suggestions.len(), 1);
    assert_eq!(result.suggestions[0].confidence, 1.0);
}

#[tokio::test]
async fn test_simple_metavariable_generation() {
    let params = SuggestPatternsParam {
        code_examples: vec![
            "console.log('hello')".to_string(),
            "console.log('world')".to_string(),
        ],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await.unwrap();

    assert_eq!(result.suggestions.len(), 1);
    assert_eq!(result.suggestions[0].pattern, "console.log($MSG)");
    assert_eq!(result.suggestions[0].specificity, SpecificityLevel::General);
}

#[tokio::test]
async fn test_function_name_metavariables() {
    let params = SuggestPatternsParam {
        code_examples: vec![
            "function getUserData() {}".to_string(),
            "function getPostData() {}".to_string(),
        ],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await.unwrap();

    assert!(
        result
            .suggestions
            .iter()
            .any(|s| s.pattern == "function $NAME() {}" || s.pattern == "function get$TYPE() {}")
    );
}

#[tokio::test]
async fn test_nested_structure_patterns() {
    let params = SuggestPatternsParam {
        code_examples: vec![
            "if (user.name === 'admin') { return true; }".to_string(),
            "if (user.email === 'test@example.com') { return false; }".to_string(),
        ],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await.unwrap();

    // Should suggest patterns for nested property access
    assert!(
        result
            .suggestions
            .iter()
            .any(|s| s.pattern.contains("user.$PROP") || s.pattern.contains("$OBJ.$PROP"))
    );
}

#[tokio::test]
async fn test_multiple_statement_patterns() {
    let params = SuggestPatternsParam {
        code_examples: vec![
            "const user = getUser(); console.log(user.name);".to_string(),
            "const data = getData(); console.log(data.value);".to_string(),
        ],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await.unwrap();

    // Should suggest patterns for multiple statements
    assert!(
        result
            .suggestions
            .iter()
            .any(|s| s.pattern.contains("const $VAR = $FUNC();")
                || s.pattern.contains("console.log($VAR.$PROP);"))
    );
}

#[tokio::test]
async fn test_ast_traversal_and_node_identification() {
    let params = SuggestPatternsParam {
        code_examples: vec![
            "class UserService { constructor() {} }".to_string(),
            "class PostService { constructor() {} }".to_string(),
        ],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await.unwrap();

    // Should suggest patterns for class declarations
    assert!(
        result
            .suggestions
            .iter()
            .any(|s| s.pattern.contains("class $NAME") || s.pattern.contains("constructor"))
    );

    // Should include node kinds for AST structure
    assert!(
        result
            .suggestions
            .iter()
            .any(|s| s.node_kinds.contains(&"class_declaration".to_string())
                || s.node_kinds.contains(&"constructor_definition".to_string()))
    );
}

#[tokio::test]
async fn test_structural_pattern_extraction() {
    let params = SuggestPatternsParam {
        code_examples: vec![
            "for (let i = 0; i < arr.length; i++) { process(arr[i]); }".to_string(),
            "for (let j = 0; j < list.length; j++) { handle(list[j]); }".to_string(),
        ],
        language: "javascript".to_string(),
        max_suggestions: Some(5),
        specificity_levels: None,
    };

    let service = AstGrepService::new();
    let result = service.suggest_patterns(params).await.unwrap();

    // Should suggest patterns for for loops with array iteration
    assert!(result.suggestions.iter().any(|s| {
        s.pattern
            .contains("for (let $VAR = 0; $VAR < $ARR.length; $VAR++)")
            || s.pattern.contains("$FUNC($ARR[$VAR])")
    }));
}
