use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::{RuleSearchParam, ast_grep_service::AstGrepService};
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

    // Create test file
    let code = r#"
class MyClass {
    debug() {
        console.log("debugging");
    }

    process() {
        this.data = "processed";
    }
}

function standalone() {
    console.log("standalone");
}
"#;

    fs::write(temp_dir.path().join("test.js"), code).unwrap();

    // Test 1: Find all method_definitions
    println!("\n=== Test 1: Find all method_definitions ===");
    let rule1 = r#"
id: test1
language: javascript
rule:
  kind: method_definition
"#;

    let param1 = RuleSearchParam {
        rule_config: rule1.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result1 = service.rule_search(param1).await.unwrap();
    println!(
        "Found {} method_definitions",
        result1
            .matches
            .iter()
            .map(|f| f.matches.len())
            .sum::<usize>()
    );
    for file in &result1.matches {
        for m in &file.matches {
            println!("  - {}", m.text.lines().next().unwrap_or(""));
        }
    }

    // Test 2: Find patterns inside class
    println!("\n=== Test 2: Find patterns inside class ===");
    let rule2 = r#"
id: test2
language: javascript
rule:
  inside:
    pattern: class $CLASS { $METHODS }
"#;

    let param2 = RuleSearchParam {
        rule_config: rule2.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result2 = service.rule_search(param2).await.unwrap();
    println!(
        "Found {} patterns inside class",
        result2
            .matches
            .iter()
            .map(|f| f.matches.len())
            .sum::<usize>()
    );
    for file in &result2.matches {
        for m in &file.matches {
            let first_line = m.text.lines().next().unwrap_or("");
            println!("  - {} (lines {}-{})", first_line, m.start_line, m.end_line);
        }
    }

    // Test 3: Combined rule - method_definition AND inside class
    println!("\n=== Test 3: method_definition AND inside class ===");
    let rule3 = r#"
id: test3
language: javascript
rule:
  all:
    - kind: method_definition
    - inside:
        pattern: class $CLASS { $METHODS }
"#;

    let param3 = RuleSearchParam {
        rule_config: rule3.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result3 = service.rule_search(param3).await.unwrap();
    println!(
        "Found {} method_definitions inside class",
        result3
            .matches
            .iter()
            .map(|f| f.matches.len())
            .sum::<usize>()
    );
    for file in &result3.matches {
        for m in &file.matches {
            println!("  - {}", m.text.lines().next().unwrap_or(""));
        }
    }
}
