use ast_grep_language::SupportLang as Language;
use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::rules::ast::Rule;
use ast_grep_mcp::rules::evaluation::RuleEvaluator;
use ast_grep_mcp::{RuleSearchParam, ast_grep_service::AstGrepService};
use std::fs;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    let code = r#"class MyClass {
    debug() {
        console.log("debugging");
    }
}
"#;

    // Test 1: Direct evaluation
    println!("=== Test 1: Direct Evaluation ===");
    let evaluator = RuleEvaluator::new();

    // Create the exact same rule structure as YAML parsing produces
    let inside_rule = Rule::Inside {
        rule: Box::new(Rule::Pattern(
            ast_grep_mcp::rules::ast::PatternRule::Simple {
                pattern: "$_".to_string(),
            },
        )),
        inside_of: Box::new(Rule::Pattern(
            ast_grep_mcp::rules::ast::PatternRule::Simple {
                pattern: "class $CLASS { $METHODS }".to_string(),
            },
        )),
    };

    let all_rule = Rule::All(vec![
        Rule::Kind("method_definition".to_string()),
        inside_rule,
    ]);

    match evaluator.evaluate_rule(&all_rule, code, Language::JavaScript) {
        Ok(matches) => {
            println!("Direct evaluation found {} matches", matches.len());
            for m in &matches {
                println!("  Match: {}", m.text.lines().next().unwrap_or(""));
            }
        }
        Err(e) => {
            println!("Direct evaluation error: {e}");
        }
    }

    // Test 2: Through service
    println!("\n=== Test 2: Through Service ===");
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    fs::write(temp_dir.path().join("test.js"), code).unwrap();

    let rule_config = r#"
id: test-rule
language: javascript
rule:
  all:
    - kind: method_definition
    - inside:
        pattern: class $CLASS { $METHODS }
"#;

    let param = RuleSearchParam {
        rule_config: rule_config.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    match service.rule_search(param).await {
        Ok(result) => {
            println!("Service found {} files with matches", result.matches.len());
            if !result.matches.is_empty() {
                for file in &result.matches {
                    println!("  File: {}", file.file_path);
                    for m in &file.matches {
                        println!("    Match: {}", m.text.lines().next().unwrap_or(""));
                    }
                }
            }
        }
        Err(e) => {
            println!("Service error: {e}");
        }
    }
}
