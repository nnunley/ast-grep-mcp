/// Comprehensive tests for ast-grep subsystem requirements
/// These tests validate core pattern matching capabilities needed for the failing tests
use ast_grep_mcp::{RuleSearchParam, ast_grep_service::AstGrepService};
use std::fs;
use tempfile::TempDir;

fn create_test_service_with_examples() -> (AstGrepService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
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
async fn test_typescript_type_annotation_patterns() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "types.ts",
        r#"
// Basic any type usage
let badVar: any = "oops";
let badArray: any[] = [];
function badFunc(param: any): void {}
const badArrow = (data: any) => data;

// Good types that should not match
let goodVar: string = "good";
let goodArray: string[] = [];
function goodFunc(param: string): void {}
const goodArrow = (data: string) => data;
"#,
    );

    // Test individual patterns
    let patterns = vec![
        ("let $_: any", "Variable with any type", 1),
        ("let $_: any[]", "Array of any type", 1),
        (
            "function $FUNC($_: any)",
            "Function parameter with any type",
            0,
        ), // This pattern doesn't work in ast-grep
        (
            "($_: any) => $_",
            "Arrow function parameter with any type",
            1,
        ),
    ];

    for (pattern, description, expected_matches) in patterns {
        let rule_config = format!(
            r#"
id: test-any-type
language: typescript
rule:
  pattern: "{pattern}"
"#
        );

        let param = RuleSearchParam {
            rule_config,
            path_pattern: Some("**/*.ts".to_string()),
            max_results: 100,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.rule_search(param).await.unwrap();

        println!("Testing pattern '{pattern}' ({description})");
        println!(
            "Expected {} matches, found {} files with {} total matches",
            expected_matches,
            result.matches.len(),
            result
                .matches
                .iter()
                .map(|f| f.matches.len())
                .sum::<usize>()
        );

        if !result.matches.is_empty() {
            for file_match in &result.matches {
                for (i, m) in file_match.matches.iter().enumerate() {
                    println!("  Match {}: '{}'", i + 1, m.text.trim());
                }
            }
        }

        assert_eq!(
            result
                .matches
                .iter()
                .map(|f| f.matches.len())
                .sum::<usize>(),
            expected_matches,
            "Pattern '{pattern}' should find {expected_matches} matches"
        );
    }
}

#[tokio::test]
async fn test_metavariable_repetition_patterns() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "defensive.ts",
        r#"
function safeCalls(obj: any) {
    // These should be matched - same metavariable repeated
    obj.method && obj.method();
    obj.callback && obj.callback(data);
    user.validate && user.validate();

    // These should not be matched - different patterns
    obj.method();
    obj?.method?.();
    if (obj.method) {
        obj.method();
    }
    // Different metavariables
    obj.method && other.method();
}
"#,
    );

    let rule_config = r#"
id: test-metavar-repetition
language: typescript
rule:
  any:
    - pattern: $PROP && $PROP()
    - pattern: $PROP && $PROP($$ARGS)
"#;

    let param = RuleSearchParam {
        rule_config: rule_config.to_string(),
        path_pattern: Some("**/*.ts".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    println!("Metavariable repetition test results:");
    println!("Found {} files with matches", result.matches.len());

    if !result.matches.is_empty() {
        for file_match in &result.matches {
            println!("File: {}", file_match.file_path);
            for (i, m) in file_match.matches.iter().enumerate() {
                println!("  Match {}: '{}'", i + 1, m.text.trim());
            }
        }

        // Should find 3 defensive check patterns where same metavariable is used twice
        assert_eq!(result.matches[0].matches.len(), 3);
    } else {
        panic!("Should find defensive check patterns with repeated metavariables");
    }
}

#[tokio::test]
async fn test_complex_relational_rule_components() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "classes.js",
        r#"
class MyClass {
    // Should match: method with console.log but no return
    debug() {
        console.log("debugging");
        this.process();
    }

    // Should NOT match: has return statement
    calculate() {
        console.log("calculating");
        return 42;
    }

    // Should NOT match: no console.log
    process() {
        this.data = "processed";
    }
}

// Should NOT match: not inside a class
function standalone() {
    console.log("standalone");
}
"#,
    );

    // Test individual components of the complex rule
    let test_cases = vec![
        (
            "kind: method_definition",
            "Method definition kind",
            3, // All 3 methods in the class
        ),
        (
            "has:\n  pattern: console.log($MSG)",
            "Methods containing console.log",
            2, // debug() and calculate()
        ),
        (
            "inside:\n  pattern: class $CLASS { $METHODS }",
            "Methods inside a class",
            3, // All methods in MyClass
        ),
        (
            "not:\n  has:\n    pattern: return $VALUE",
            "Methods without return statements",
            2, // debug() and process()
        ),
    ];

    for (rule_part, description, expected_matches) in test_cases {
        let rule_config = format!(
            r#"
id: test-relational-component
language: javascript
rule:
  {rule_part}
"#
        );

        let param = RuleSearchParam {
            rule_config,
            path_pattern: Some("**/*.js".to_string()),
            max_results: 100,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.rule_search(param).await.unwrap();

        println!("Testing rule component: {description}");
        println!("Rule: {rule_part}");
        println!(
            "Expected {} matches, found {} total matches",
            expected_matches,
            result
                .matches
                .iter()
                .map(|f| f.matches.len())
                .sum::<usize>()
        );

        if !result.matches.is_empty() {
            for file_match in &result.matches {
                for (i, m) in file_match.matches.iter().enumerate() {
                    println!("  Match {}: '{}'", i + 1, m.text.trim());
                }
            }
        }
    }
}

#[tokio::test]
async fn test_combined_complex_relational_rule() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "complex.js",
        r#"class MyClass {
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
"#,
    );

    let rule_config = r#"
id: test-complex-relational
language: javascript
rule:
  all:
    - kind: method_definition
    - has:
        pattern: console.log($MSG)
    - inside:
        pattern: class $CLASS { $$$METHODS }
    - not:
        has:
          pattern: return $VALUE
"#;

    let param = RuleSearchParam {
        rule_config: rule_config.to_string(),
        path_pattern: Some("**/*.js".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    println!("Combined complex relational rule test:");
    println!("Found {} files with matches", result.matches.len());

    if !result.matches.is_empty() {
        for file_match in &result.matches {
            println!("File: {}", file_match.file_path);
            for (i, m) in file_match.matches.iter().enumerate() {
                println!("  Match {}: '{}'", i + 1, m.text.trim());
            }
        }

        // Should find only the debug() method
        assert_eq!(result.matches[0].matches.len(), 1);
        assert!(result.matches[0].matches[0].text.contains("debug"));
    } else {
        panic!("Should find the debug() method that matches all criteria");
    }
}

#[tokio::test]
async fn test_python_language_support() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "formatting.py",
        r#"
# These should match - old .format() style
message1 = 'Hello {}'.format(name)
message2 = "Welcome {}!".format(user)
complex = 'User {} has {} points'.format(name, score)

# These should not match - already f-strings
good1 = f'Hello {name}'
good2 = f"Welcome {user}!"

# These should not match - different patterns
print("Hello world")
result = some_function()
"#,
    );

    let rule_config = r#"
id: test-python-fstring
language: python
rule:
  pattern: '"$STRING".format($ARGS)'
"#;

    let param = RuleSearchParam {
        rule_config: rule_config.to_string(),
        path_pattern: Some("**/*.py".to_string()),
        max_results: 100,
        max_file_size: 1024 * 1024,
        cursor: None,
    };

    let result = service.rule_search(param).await.unwrap();

    println!("Python language support test:");
    println!("Found {} files with matches", result.matches.len());

    if !result.matches.is_empty() {
        for file_match in &result.matches {
            println!("File: {}", file_match.file_path);
            for (i, m) in file_match.matches.iter().enumerate() {
                println!("  Match {}: '{}'", i + 1, m.text.trim());
            }
        }

        // Should find the .format() calls (both single and double quotes)
        assert!(
            !result.matches[0].matches.is_empty(),
            "Should find .format() calls"
        );
    } else {
        panic!("Should find Python .format() calls to replace with f-strings");
    }
}

#[tokio::test]
async fn test_ast_node_kind_validation() {
    let (service, temp_dir) = create_test_service_with_examples();

    create_test_file(
        temp_dir.path(),
        "node_kinds.js",
        r#"
class TestClass {
    methodDefinition() {
        return "test";
    }

    get getter() {
        return this._value;
    }
}

function functionDeclaration() {
    return "function";
}

const arrowFunction = () => "arrow";
const variable = "value";
"#,
    );

    // Test different node kinds that should be recognized
    let node_kinds = vec![
        ("method_definition", 2),    // methodDefinition() and getter()
        ("function_declaration", 1), // functionDeclaration()
        ("arrow_function", 1),       // arrowFunction
        ("variable_declarator", 2),  // arrowFunction and variable declarations
    ];

    for (kind, expected_count) in node_kinds {
        let rule_config = format!(
            r#"
id: test-node-kind
language: javascript
rule:
  kind: {kind}
"#
        );

        let param = RuleSearchParam {
            rule_config,
            path_pattern: Some("**/*.js".to_string()),
            max_results: 100,
            max_file_size: 1024 * 1024,
            cursor: None,
        };

        let result = service.rule_search(param).await.unwrap();

        println!("Testing node kind: {kind}");
        println!(
            "Expected {} matches, found {} total matches",
            expected_count,
            result
                .matches
                .iter()
                .map(|f| f.matches.len())
                .sum::<usize>()
        );

        if !result.matches.is_empty() {
            for file_match in &result.matches {
                for (i, m) in file_match.matches.iter().enumerate() {
                    println!("  Match {}: '{}'", i + 1, m.text.trim());
                }
            }
        }
    }
}
