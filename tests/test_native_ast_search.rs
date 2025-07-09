//! Tests demonstrating the benefits of using native AST nodes.

use ast_grep_mcp::embedded::EmbeddedService;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::{EmbeddedLanguageConfig, EmbeddedSearchParam};

fn create_embedded_service() -> EmbeddedService {
    EmbeddedService::new(PatternMatcher::new())
}

#[tokio::test]
async fn test_native_ast_context() {
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
        <head>
            <title>Test Page</title>
        </head>
        <body>
            <script>
                console.log("Hello World");
                alert("Test");
            </script>
            <div id="content">
                <script>
                    console.log("Another block");
                </script>
            </div>
        </body>
    </html>
    "#;

    let param = EmbeddedSearchParam {
        code: html_code.to_string(),
        pattern: "console.log($ARG)".to_string(),
        embedded_config: config,
        strictness: None,
    };

    // Use the native AST search method
    let result = service.search_embedded_native(param).await.unwrap();

    assert_eq!(result.host_language, "html");
    assert_eq!(result.embedded_language, "javascript");
    assert_eq!(result.total_embedded_blocks, 2);
    assert_eq!(result.matches.len(), 2);

    // The native version provides better context information
    // Check that the first match has proper context
    let first_match = &result.matches[0];
    assert!(first_match.host_context.contains("Block 1"));

    // Check the second match
    let second_match = &result.matches[1];
    assert!(second_match.host_context.contains("Block 2"));
}

#[tokio::test]
async fn test_native_ast_traversal() {
    // This test would demonstrate traversing the AST to find parent elements
    // For example, finding which HTML element contains a script tag

    let service = create_embedded_service();

    let config = EmbeddedLanguageConfig {
        host_language: "html".to_string(),
        embedded_language: "javascript".to_string(),
        extraction_pattern: "<script>$CODE</script>".to_string(),
        selector: None,
        context: None,
    };

    let html_code = r#"
    <div class="container">
        <script>
            document.getElementById("test").innerHTML = "Modified";
        </script>
    </div>
    "#;

    let param = EmbeddedSearchParam {
        code: html_code.to_string(),
        pattern: "document.getElementById($ID)".to_string(),
        embedded_config: config,
        strictness: None,
    };

    let result = service.search_embedded_native(param).await.unwrap();

    assert!(!result.matches.is_empty());
    // With native AST, we could potentially traverse to find the parent div
    // and report that the script is inside a div with class "container"
}
