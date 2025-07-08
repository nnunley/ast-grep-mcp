use ast_grep_mcp::RuleSearchParam;
use ast_grep_mcp::ast_grep_service::AstGrepService;
use ast_grep_mcp::config::ServiceConfig;
use std::fs;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    // Create the exact test file
    let code = r#"class MyClass {
    // This should match: method_definition inside class with console.log but no return
    debug() {
        console.log("debugging");
        this.process();
    }

    // This should NOT match: has return statement
    calculate() {
        console.log("calculating");
        return 42;
    }

    // This should NOT match: no console.log
    process() {
        this.data = "processed";
    }
}

// This should NOT match: not inside a class
function standalone() {
    console.log("standalone");
}
"#;

    fs::write(temp_dir.path().join("complex.js"), code).unwrap();

    // Test just the class pattern
    let rule_config = r#"
id: test-class
language: javascript
rule:
  pattern: class $CLASS { $METHODS }
"#;

    let param = RuleSearchParam {
        rule_config: rule_config.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();
    println!("Class pattern search:");
    println!("Found {} files with matches", result.matches.len());

    if !result.matches.is_empty() {
        for file_match in &result.matches {
            println!("\nFile: {}", file_match.file_path);
            for (i, m) in file_match.matches.iter().enumerate() {
                println!(
                    "  Match {} (lines {}-{}): '{}'",
                    i + 1,
                    m.start_line,
                    m.end_line,
                    m.text.lines().next().unwrap_or("")
                );
            }
        }
    }
}
