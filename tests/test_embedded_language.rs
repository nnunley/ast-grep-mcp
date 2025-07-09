//! Comprehensive tests for embedded language functionality.

use ast_grep_mcp::embedded::EmbeddedService;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::{EmbeddedLanguageConfig, EmbeddedSearchParam};

fn create_embedded_service() -> EmbeddedService {
    EmbeddedService::new(PatternMatcher::new())
}

#[tokio::test]
async fn test_javascript_in_html_search() {
    let service = create_embedded_service();

    let config = EmbeddedLanguageConfig {
        host_language: "html".to_string(),
        embedded_language: "javascript".to_string(),
        extraction_pattern: "<script>$CODE</script>".to_string(),
        selector: None,
        context: None,
    };

    let html_code = r#"
    <html>
        <script>
            console.log("Hello World");
            alert("Test");
        </script>
    </html>
    "#;

    let param = EmbeddedSearchParam {
        code: html_code.to_string(),
        pattern: "console.log($ARG)".to_string(),
        embedded_config: config,
        strictness: None,
    };

    let result = service.search_embedded(param).await.unwrap();
    println!("Result: {result:?}");

    assert_eq!(result.host_language, "html");
    assert_eq!(result.embedded_language, "javascript");
    // This test will help us debug what's happening
    assert!(result.total_embedded_blocks > 0);
}

#[tokio::test]
async fn test_simple_javascript_extraction() {
    let service = create_embedded_service();

    let config = EmbeddedLanguageConfig {
        host_language: "javascript".to_string(), // Use JS as host to parse the JS code
        embedded_language: "javascript".to_string(),
        extraction_pattern: "console.log($ARG)".to_string(), // Extract specific patterns
        selector: None,
        context: None,
    };

    let js_code = r#"console.log("Hello World");"#;

    let param = EmbeddedSearchParam {
        code: js_code.to_string(),
        pattern: "console.log($ARG)".to_string(),
        embedded_config: config,
        strictness: None,
    };

    let result = service.search_embedded(param).await.unwrap();
    println!("Simple JS Result: {result:?}");

    assert_eq!(result.total_embedded_blocks, 1);
    assert!(!result.matches.is_empty(), "Should find console.log match");
}

#[tokio::test]
async fn test_python_docstring_search() {
    let service = create_embedded_service();

    // Search for Python code within Python docstrings (a real use case)
    let config = EmbeddedLanguageConfig {
        host_language: "python".to_string(),
        embedded_language: "python".to_string(),
        extraction_pattern: r#""""$CODE""""#.to_string(),
        selector: None,
        context: None,
    };

    let python_code = r#"
def example():
    """
    Example usage:

    >>> result = compute(10, 20)
    >>> print(result)
    30
    """
    return None
    "#;

    let param = EmbeddedSearchParam {
        code: python_code.to_string(),
        pattern: "print($ARG)".to_string(),
        embedded_config: config,
        strictness: None,
    };

    let result = service.search_embedded(param).await.unwrap();

    assert_eq!(result.host_language, "python");
    assert_eq!(result.embedded_language, "python");
    assert!(result.total_embedded_blocks > 0);
}
