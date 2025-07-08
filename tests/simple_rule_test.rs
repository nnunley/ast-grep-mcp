use ast_grep_mcp::{RuleSearchParam, ast_grep_service::AstGrepService};
use std::fs;

#[tokio::test]
async fn test_simple_rule_search() {
    let service = AstGrepService::new();

    // Create a temporary test file in the current directory
    let test_content = r#"
function test() {
    console.log("Hello World");
}
"#;

    // Write test file to current directory
    fs::write("temp_test.js", test_content).unwrap();

    // Create a simple rule
    let rule_config = r#"
id: test-console
language: javascript
rule:
  pattern: console.log($ARGS)
"#;

    let param = RuleSearchParam {
        rule_config: rule_config.to_string(),
        path_pattern: Some("temp_test.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    // Clean up
    let _ = fs::remove_file("temp_test.js");

    println!("Found {} files with matches", result.matches.len());
    for (i, file_match) in result.matches.iter().enumerate() {
        println!(
            "File {}: {} ({} matches)",
            i,
            file_match.file_path,
            file_match.matches.len()
        );
        for (j, match_result) in file_match.matches.iter().enumerate() {
            println!("  Match {}: {}", j, match_result.text);
        }
    }

    // Should find 1 file with 1 console.log match
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 1);
    assert!(result.matches[0].matches[0].text.contains("console.log"));
}
