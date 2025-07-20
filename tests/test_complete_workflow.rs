//! Complete workflow integration test for sgconfig.yml support

use ast_grep_mcp::{
    FileSearchParam, ListRulesParam, RuleReplaceParam, RuleSearchParam,
    ast_grep_service::AstGrepService, config::ServiceConfig,
};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
#[ignore = "TODO: Implement file replacement logic"]
async fn test_complete_sgconfig_workflow() {
    // Setup a realistic project structure
    let project_dir = TempDir::new().unwrap();

    // Create project directories
    let src_dir = project_dir.path().join("src");
    let lib_dir = src_dir.join("lib");
    let tests_dir = project_dir.path().join("tests");
    let company_rules = project_dir.path().join("company-rules");
    let team_rules = project_dir.path().join("team-specific-rules");

    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&lib_dir).unwrap();
    fs::create_dir_all(&tests_dir).unwrap();
    fs::create_dir_all(&company_rules).unwrap();
    fs::create_dir_all(&team_rules).unwrap();

    // Create sgconfig.yml with multiple rule directories
    let sgconfig_content = r#"
# Project configuration for ast-grep
ruleDirs:
  - ./company-rules        # Company-wide standards
  - ./team-specific-rules  # Team-specific conventions
utilDirs:
  - ./utils
"#;
    fs::write(project_dir.path().join("sgconfig.yml"), sgconfig_content).unwrap();

    // Create some source files with various patterns
    let main_js = r#"
// Main application file
import { logger } from './lib/logger';

function main() {
    console.log("Starting application");

    try {
        const result = processData();
        console.log("Result:", result);
    } catch (error) {
        console.error("Error occurred:", error);
        debugger; // Remove in production
    }
}

function processData() {
    // TODO: Remove debug logging
    console.debug("Processing data");
    const data = fetchData();

    if (data == null) { // Should use ===
        console.error("No data found");
        return null;
    }

    return data;
}
"#;
    fs::write(src_dir.join("main.js"), main_js).unwrap();

    let logger_js = r#"
// Logger module
export const logger = {
    log: (msg) => console.log(msg),
    error: (msg) => console.error(msg),
    debug: (msg) => console.debug(msg),
    warn: (msg) => console.warn(msg)
};

// Legacy function - should be removed
function oldLog(message) {
    console.log("DEPRECATED:", message);
}
"#;
    fs::write(lib_dir.join("logger.js"), logger_js).unwrap();

    // Create company-wide rules
    let no_debugger_rule = r#"
id: no-debugger-statements
message: Remove debugger statements before production
severity: error
language: javascript
rule:
  pattern: debugger
fix: |
  // debugger removed
"#;
    fs::write(company_rules.join("no-debugger.yaml"), no_debugger_rule).unwrap();

    let strict_equality_rule = r#"
id: use-strict-equality
message: Use strict equality (===) instead of loose equality (==)
severity: warning
language: javascript
rule:
  pattern: $A == $B
  not:
    any:
      - pattern: $A == null
      - pattern: null == $A
fix: |
  $A === $B
"#;
    fs::write(
        company_rules.join("strict-equality.yaml"),
        strict_equality_rule,
    )
    .unwrap();

    // Create team-specific rules
    let no_console_debug_rule = r#"
id: no-console-debug
message: Remove console.debug statements
severity: warning
language: javascript
rule:
  pattern: console.debug($$$)
fix: |
  // console.debug($$$)
"#;
    fs::write(
        team_rules.join("no-console-debug.yaml"),
        no_console_debug_rule,
    )
    .unwrap();

    let deprecation_rule = r#"
id: remove-deprecated-functions
message: Remove deprecated functions
severity: error
language: javascript
rule:
  kind: function_declaration
  has:
    kind: comment
    regex: "DEPRECATED"
"#;
    fs::write(team_rules.join("deprecation.yaml"), deprecation_rule).unwrap();

    // Initialize service with sgconfig.yml discovery
    let config = ServiceConfig {
        root_directories: vec![project_dir.path().to_path_buf()],
        rules_directory: project_dir.path().join(".ast-grep-rules"), // Local project rules
        ..Default::default()
    }
    .with_sg_config(None);

    let service = AstGrepService::with_config(config);

    // Test 1: File search for debugger statements
    let search_param = FileSearchParam {
        pattern: "debugger".to_string(),
        language: "javascript".to_string(),
        path_pattern: "**/*.js".to_string(),
        max_results: 10,
        max_file_size: 1024 * 1024,
        cursor: None,
        strictness: None,
        selector: None,
        context: None,
        context_before: Some(2),
        context_after: Some(2),
        context_lines: None,
    };

    let search_result = service.file_search(search_param).await.unwrap();
    assert_eq!(search_result.matches.len(), 1);
    assert!(search_result.matches[0].file_path.ends_with("src/main.js"));
    assert_eq!(search_result.matches[0].matches.len(), 1);

    // Test 2: Rule search for all issues
    let no_debugger_config = fs::read_to_string(company_rules.join("no-debugger.yaml")).unwrap();

    let rule_search_param = RuleSearchParam {
        rule_config: no_debugger_config.clone(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let rule_search_result = service.rule_search(rule_search_param).await.unwrap();
    assert_eq!(rule_search_result.matches.len(), 1);

    // Test 3: Apply fixes (dry run first)
    let rule_replace_param = RuleReplaceParam {
        rule_config: no_debugger_config,
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        dry_run: true,
        summary_only: false,
        cursor: None,
    };

    let replace_result = service.rule_replace(rule_replace_param).await.unwrap();
    assert!(replace_result.dry_run);
    assert_eq!(replace_result.files_with_changes, 1);
    assert_eq!(replace_result.total_changes, 1);

    // Verify the change details
    let file_result = &replace_result.file_results[0];
    assert!(file_result.file_path.ends_with("src/main.js"));
    assert_eq!(file_result.changes.len(), 1);
    assert!(file_result.changes[0].old_text.contains("debugger"));
    assert!(
        file_result.changes[0]
            .new_text
            .contains("// debugger removed")
    );

    // Test 4: Search for equality issues
    let equality_search = FileSearchParam {
        pattern: "$A == $B".to_string(),
        language: "javascript".to_string(),
        path_pattern: "**/*.js".to_string(),
        max_results: 10,
        max_file_size: 1024 * 1024,
        cursor: None,
        strictness: None,
        selector: None,
        context: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let equality_result = service.file_search(equality_search).await.unwrap();
    assert_eq!(equality_result.matches.len(), 1); // Only main.js has == usage
    assert_eq!(equality_result.matches[0].matches.len(), 1); // data == null

    // Test 5: Verify rules from both directories are available
    let list_param = ListRulesParam {
        language: None,
        severity: None,
    };
    let list_result = service.list_rules(list_param).await.unwrap();

    // Should have rules from both company-rules and team-specific-rules
    let rule_ids: Vec<&str> = list_result.rules.iter().map(|r| r.id.as_str()).collect();
    assert!(rule_ids.contains(&"no-debugger-statements"));
    assert!(rule_ids.contains(&"use-strict-equality"));
    assert!(rule_ids.contains(&"no-console-debug"));
    assert!(rule_ids.contains(&"remove-deprecated-functions"));

    // Verify rules are from correct directories
    for rule in &list_result.rules {
        match rule.id.as_str() {
            "no-debugger-statements" | "use-strict-equality" => {
                assert!(rule.file_path.contains("company-rules"));
            }
            "no-console-debug" | "remove-deprecated-functions" => {
                assert!(rule.file_path.contains("team-specific-rules"));
            }
            _ => panic!("Unexpected rule: {}", rule.id),
        }
    }

    println!("✅ Complete workflow test passed!");
    println!("   - sgconfig.yml discovered and loaded");
    println!("   - Rules from multiple directories working");
    println!("   - File search, rule search, and rule replace all functional");
    println!(
        "   - {} total rules loaded from {} directories",
        list_result.rules.len(),
        2
    );
}
