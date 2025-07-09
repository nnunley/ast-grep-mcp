use ast_grep_mcp::debug::DebugService;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::{DebugAstParam, DebugFormat, DebugPatternParam};

fn create_debug_service() -> DebugService {
    DebugService::new(PatternMatcher::new())
}

#[tokio::test]
async fn test_debug_pattern_analysis() {
    let service = create_debug_service();

    let param = DebugPatternParam {
        pattern: "function $NAME($ARGS) { $$$ }".to_string(),
        language: "javascript".to_string(),
        sample_code: Some("function test(a, b) { return a + b; }".to_string()),
        format: DebugFormat::Pattern,
    };

    let result = service.debug_pattern(param).await.unwrap();

    assert_eq!(result.pattern, "function $NAME($ARGS) { $$$ }");
    assert_eq!(result.language, "javascript");
    assert_eq!(result.format, DebugFormat::Pattern);
    assert!(result.debug_info.contains("Pattern Analysis"));
    assert!(result.debug_info.contains("$NAME"));
    assert!(result.debug_info.contains("$ARGS"));
    assert!(result.debug_info.contains("$$$"));
    assert!(result.explanation.contains("matches"));
    assert!(result.sample_matches.is_some());
}

#[tokio::test]
async fn test_debug_pattern_metavar_extraction() {
    let service = create_debug_service();

    let param = DebugPatternParam {
        pattern: "console.log($MESSAGE)".to_string(),
        language: "javascript".to_string(),
        sample_code: Some("console.log('Hello World')".to_string()),
        format: DebugFormat::Pattern,
    };

    let result = service.debug_pattern(param).await.unwrap();

    assert!(result.debug_info.contains("$MESSAGE"));
    assert!(result.explanation.contains("$MESSAGE"));
    assert!(result.sample_matches.is_some());
    let matches = result.sample_matches.unwrap();
    assert!(!matches.is_empty());
}

#[tokio::test]
async fn test_debug_pattern_ast_format() {
    let service = create_debug_service();

    let param = DebugPatternParam {
        pattern: "if ($CONDITION) { $BODY }".to_string(),
        language: "javascript".to_string(),
        sample_code: None,
        format: DebugFormat::Ast,
    };

    let result = service.debug_pattern(param).await.unwrap();

    assert_eq!(result.format, DebugFormat::Ast);
    assert!(result.debug_info.contains("if")); // Should show AST structure
}

#[tokio::test]
async fn test_debug_ast_basic() {
    let service = create_debug_service();

    let param = DebugAstParam {
        code: "function test() { return 42; }".to_string(),
        language: "javascript".to_string(),
        format: DebugFormat::Ast,
        include_trivia: false,
    };

    let result = service.debug_ast(param).await.unwrap();

    assert_eq!(result.language, "javascript");
    assert_eq!(result.format, DebugFormat::Ast);
    assert_eq!(result.code_length, 30);
    assert!(result.tree.contains("function"));
    assert!(!result.node_kinds.is_empty());
    assert!(result.tree_stats.total_nodes > 0);
    assert_eq!(result.tree_stats.error_nodes, 0);
}

#[tokio::test]
async fn test_debug_ast_cst_format() {
    let service = create_debug_service();

    let param = DebugAstParam {
        code: "let x = 1;".to_string(),
        language: "javascript".to_string(),
        format: DebugFormat::Cst,
        include_trivia: true,
    };

    let result = service.debug_ast(param).await.unwrap();

    assert_eq!(result.format, DebugFormat::Cst);
    assert!(result.tree.contains("let"));
    assert!(result.tree_stats.total_nodes > 0);
    // CST should generally have more nodes than AST due to trivia
}

#[tokio::test]
async fn test_debug_pattern_with_ellipsis() {
    let service = create_debug_service();

    let param = DebugPatternParam {
        pattern: "class $NAME { $$$ }".to_string(),
        language: "javascript".to_string(),
        sample_code: Some("class MyClass { method1() {} method2() {} }".to_string()),
        format: DebugFormat::Pattern,
    };

    let result = service.debug_pattern(param).await.unwrap();

    assert!(result.debug_info.contains("$$$"));
    assert!(result.debug_info.contains("multi-statement ellipsis"));
    assert!(result.explanation.contains("sequence of statements"));

    if let Some(matches) = result.sample_matches {
        assert!(!matches.is_empty());
    }
}

#[tokio::test]
async fn test_debug_pattern_with_anonymous_wildcard() {
    let service = create_debug_service();

    let param = DebugPatternParam {
        pattern: "call($_)".to_string(),
        language: "javascript".to_string(),
        sample_code: Some("call(123); call('test');".to_string()),
        format: DebugFormat::Pattern,
    };

    let result = service.debug_pattern(param).await.unwrap();

    assert!(result.debug_info.contains("$_"));
    assert!(result.debug_info.contains("anonymous wildcards"));
    assert!(result.explanation.contains("any single expression"));
}

#[tokio::test]
async fn test_debug_ast_with_errors() {
    let service = create_debug_service();

    // Invalid JavaScript code to test error node detection
    let param = DebugAstParam {
        code: "function test() { return; } extra stuff".to_string(),
        language: "javascript".to_string(),
        format: DebugFormat::Ast,
        include_trivia: false,
    };

    let result = service.debug_ast(param).await.unwrap();

    assert!(result.tree_stats.total_nodes > 0);
    // The error node count depends on how tree-sitter parses the invalid code
}

#[tokio::test]
async fn test_debug_pattern_no_metavars() {
    let service = create_debug_service();

    let param = DebugPatternParam {
        pattern: "console.log('literal')".to_string(),
        language: "javascript".to_string(),
        sample_code: Some("console.log('literal')".to_string()),
        format: DebugFormat::Pattern,
    };

    let result = service.debug_pattern(param).await.unwrap();

    // The pattern doesn't contain metavariables, so it should be identified as literal
    assert!(result.explanation.contains("Exact literal text matches"));
    assert!(!result.debug_info.contains("Metavariables found")); // Should not have metavars section
}

#[tokio::test]
async fn test_debug_ast_node_kinds_extraction() {
    let service = create_debug_service();

    let param = DebugAstParam {
        code: "const arr = [1, 2, 3]; arr.push(4);".to_string(),
        language: "javascript".to_string(),
        format: DebugFormat::Ast,
        include_trivia: false,
    };

    let result = service.debug_ast(param).await.unwrap();

    // Should extract various node kinds
    assert!(result.node_kinds.len() > 5);
    assert!(
        result
            .node_kinds
            .iter()
            .any(|kind| kind.contains("variable"))
    );
    assert!(result.node_kinds.iter().any(|kind| kind.contains("array")));

    // Check that node kinds are sorted
    let mut sorted_kinds = result.node_kinds.clone();
    sorted_kinds.sort();
    assert_eq!(result.node_kinds, sorted_kinds);
}
