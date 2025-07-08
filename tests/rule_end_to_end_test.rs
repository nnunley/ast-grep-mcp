use ast_grep_mcp::{
    RuleReplaceParam, RuleSearchParam, RuleValidateParam, ast_grep_service::AstGrepService,
};
use std::fs;
use tempfile::TempDir;

fn create_test_service_with_examples() -> (AstGrepService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    // Create service with temp directory as root to isolate tests
    let config = ast_grep_mcp::config::ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    (service, temp_dir)
}

fn create_test_file(dir: &std::path::Path, name: &str, content: &str) {
    let file_path = dir.join(name);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(file_path, content).unwrap();
}

#[tokio::test]
async fn test_console_to_logger_rule_search() {
    let (service, temp_dir) = create_test_service_with_examples();

    // Create test JavaScript file with console.log statements
    create_test_file(
        temp_dir.path(),
        "unique_test_file.js",
        r#"
function greet(name) {
    console.log("Hello " + name);
    console.error("This should not match");
    console.log("Debug: function called");
}

class Logger {
    info(msg) {
        console.log("Info: " + msg);
    }
}
"#,
    );

    // Load the console-to-logger rule
    let rule_config = fs::read_to_string("examples/rules/console-to-logger.yaml").unwrap();

    let param = RuleSearchParam {
        rule_config,
        path_pattern: Some(format!("{}/**/*.js", temp_dir.path().display())),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    // Should find 3 console.log matches (not console.error)
    assert_eq!(result.matches.len(), 1);
    let file_result = &result.matches[0];
    assert_eq!(file_result.matches.len(), 3);

    // Check that all matches are console.log
    for match_result in &file_result.matches {
        assert!(match_result.text.contains("console.log"));
        assert!(!match_result.text.contains("console.error"));
    }
}

#[tokio::test]
async fn test_console_to_logger_rule_replace() {
    let (service, temp_dir) = create_test_service_with_examples();

    // Create test JavaScript file
    let original_content = r#"
function debug() {
    console.log("Starting process");
    console.log("Process complete");
}
"#;
    create_test_file(temp_dir.path(), "debug.js", original_content);

    // Load the console-to-logger rule
    let rule_config = fs::read_to_string("examples/rules/console-to-logger.yaml").unwrap();

    let param = RuleReplaceParam {
        rule_config,
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        dry_run: false, // Actually perform the replacement
        summary_only: false,
        cursor: None,
    };

    let result = service.rule_replace(param).await.unwrap();

    // Should find and replace console.log with logger.log
    assert_eq!(result.file_results.len(), 1);
    let file_result = &result.file_results[0];
    assert_eq!(file_result.total_changes, 2);

    // Verify the file was actually modified
    let modified_content = fs::read_to_string(temp_dir.path().join("debug.js")).unwrap();
    assert!(modified_content.contains("logger.log"));
    assert!(!modified_content.contains("console.log"));
}

#[tokio::test]
async fn test_var_to_let_rule() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "legacy.js",
        r#"
function oldCode() {
    var x = 1;
    var message = "hello";
    let alreadyGood = true;
    const CONSTANT = 42;
    var y = x + 1;
}
"#,
    );

    let rule_config = fs::read_to_string("examples/rules/var-to-let.yaml").unwrap();

    let search_param = RuleSearchParam {
        rule_config: rule_config.clone(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let search_result = service.rule_search(search_param).await.unwrap();

    // Should find 3 var declarations
    assert_eq!(search_result.matches.len(), 1);
    assert_eq!(search_result.matches[0].matches.len(), 3);

    // Test replacement
    let replace_param = RuleReplaceParam {
        rule_config,
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        dry_run: true, // Just preview
        summary_only: false,
        cursor: None,
    };

    let replace_result = service.rule_replace(replace_param).await.unwrap();
    assert_eq!(replace_result.total_changes, 3);
}

#[tokio::test]
async fn test_optional_chaining_typescript_rule() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "defensive.ts",
        r#"
function safeCalls(obj: any) {
    // These should be matched and replaced
    obj.method && obj.method();
    obj.callback && obj.callback(data);

    // These should not be matched
    obj.method();
    obj?.method?.();
    if (obj.method) {
        obj.method();
    }
}
"#,
    );

    let rule_config = fs::read_to_string("examples/rules/optional-chaining.yaml").unwrap();

    let param = RuleSearchParam {
        rule_config,
        path_pattern: Some("**/*.ts".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    // Should find 2 defensive check patterns
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 2);

    for match_result in &result.matches[0].matches {
        assert!(match_result.text.contains("&&"));
        assert!(match_result.text.contains("()"));
    }
}

#[tokio::test]
async fn test_nested_ternary_detection() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "complex.js",
        r#"
function complexLogic(a, b, c, d) {
    // This should be detected - nested ternary
    const result1 = a ? (b ? c : d) : e;

    // This should also be detected
    const result2 = condition ? (inner ? yes : no) : defaultValue;

    // These should not be detected - simple ternary
    const simple = a ? b : c;
    const chained = a ? b : c ? d : e; // Different pattern
}
"#,
    );

    let rule_config = fs::read_to_string("examples/rules/no-nested-ternary.yaml").unwrap();

    let param = RuleSearchParam {
        rule_config,
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    // Should find 2 nested ternary patterns
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 2);
}

#[tokio::test]
async fn test_typescript_any_type_detection() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "types.ts",
        r#"
// These should be detected
let badVar: any = "oops";
let badArray: any[] = [];
function badFunc(param: any): void {}
const badArrow = (data: any) => data;

// These should not be detected
let goodVar: string = "good";
let goodArray: string[] = [];
function goodFunc(param: string): void {}
const goodArrow = (data: string) => data;
"#,
    );

    let rule_config = fs::read_to_string("examples/rules/no-any-type.yaml").unwrap();

    let param = RuleSearchParam {
        rule_config,
        path_pattern: Some("**/*.ts".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    // Should find 4 any type usages
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 4);

    for match_result in &result.matches[0].matches {
        assert!(match_result.text.contains("any"));
    }
}

#[tokio::test]
async fn test_complex_relational_rule() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "complex.js",
        r#"
class MyClass {
    // This should match: function inside class with console.log but no return
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
"#,
    );

    let rule_config = fs::read_to_string("examples/rules/complex-relational.yaml").unwrap();

    let param = RuleSearchParam {
        rule_config,
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    // Should find only the debug() method
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 1);

    let match_result = &result.matches[0].matches[0];
    assert!(match_result.text.contains("debug"));
    assert!(match_result.text.contains("console.log"));
    assert!(!match_result.text.contains("return"));
}

#[tokio::test]
async fn test_multiple_languages() {
    let (service, temp_dir) = create_test_service_with_examples();

    // Create files in different languages
    create_test_file(temp_dir.path(), "test.js", "console.log('js');");
    create_test_file(temp_dir.path(), "test.ts", "obj.method && obj.method();");
    create_test_file(temp_dir.path(), "test.py", "'Hello {}'.format(name)");

    // Test JavaScript rule
    let js_rule = fs::read_to_string("examples/rules/console-to-logger.yaml").unwrap();
    let js_result = service
        .rule_search(RuleSearchParam {
            rule_config: js_rule,
            path_pattern: Some("**/*.js".to_string()),
            max_results: 100,
            max_file_size: 1024 * 1024,
            cursor: None,
        })
        .await
        .unwrap();

    // Test TypeScript rule
    let ts_rule = fs::read_to_string("examples/rules/optional-chaining.yaml").unwrap();
    let ts_result = service
        .rule_search(RuleSearchParam {
            rule_config: ts_rule,
            path_pattern: Some("**/*.ts".to_string()),
            max_results: 100,
            max_file_size: 1024 * 1024,
            cursor: None,
        })
        .await
        .unwrap();

    // Test Python rule
    let py_rule = fs::read_to_string("examples/rules/python-fstring.yaml").unwrap();
    let py_result = service
        .rule_search(RuleSearchParam {
            rule_config: py_rule,
            path_pattern: Some("**/*.py".to_string()),
            max_results: 100,
            max_file_size: 1024 * 1024,
            cursor: None,
        })
        .await
        .unwrap();

    // Each should find matches in their respective languages
    assert_eq!(js_result.matches.len(), 1);
    assert_eq!(ts_result.matches.len(), 1);
    assert_eq!(py_result.matches.len(), 1);
}

#[tokio::test]
async fn test_rule_validation() {
    let (service, _) = create_test_service_with_examples();

    // Test valid rule
    let valid_rule = fs::read_to_string("examples/rules/console-to-logger.yaml").unwrap();
    let valid_result = service
        .validate_rule(RuleValidateParam {
            rule_config: valid_rule,
            test_code: Some("console.log('test');".to_string()),
        })
        .await
        .unwrap();

    assert!(valid_result.valid);
    assert!(valid_result.errors.is_empty());

    // Test invalid rule
    let invalid_rule = r#"
id: invalid
language: nonexistent
rule:
  invalid_field: "bad"
"#;
    let invalid_result = service
        .validate_rule(RuleValidateParam {
            rule_config: invalid_rule.to_string(),
            test_code: None,
        })
        .await
        .unwrap();

    assert!(!invalid_result.valid);
    assert!(!invalid_result.errors.is_empty());
}
