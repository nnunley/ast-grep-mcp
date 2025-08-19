//! Common test utilities
//! 
//! This module contains shared test utilities and helper functions that were
//! originally scattered across different test files.

use ast_grep_mcp::search::SearchService;
use ast_grep_mcp::replace::ReplaceService;
use ast_grep_mcp::config::ServiceConfig;
use ast_grep_mcp::pattern::PatternMatcher;
use ast_grep_mcp::rules::RuleEvaluator;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Create a test SearchService with a temporary directory
pub fn create_test_search_service() -> (SearchService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();

    (
        SearchService::new(config, pattern_matcher, rule_evaluator),
        temp_dir,
    )
}

/// Create a test ReplaceService with a temporary directory
pub fn create_test_replace_service() -> (ReplaceService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let pattern_matcher = PatternMatcher::new();
    let rule_evaluator = RuleEvaluator::new();

    (
        ReplaceService::new(config, pattern_matcher, rule_evaluator),
        temp_dir,
    )
}

/// Create a test file in the given directory
pub fn create_test_file(dir: &Path, name: &str, content: &str) {
    let file_path = dir.join(name);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(file_path, content).unwrap();
}

/// Sample JavaScript code for testing
pub fn sample_javascript_code() -> &'static str {
    r#"
function greet(name) {
    console.log("Hello, " + name);
    console.warn("This is a warning");
    return "greeting: " + name;
}

function calculate(a, b) {
    let result = a + b;
    console.log("Result: " + result);
    return result;
}

const processData = (data) => {
    return data.map(item => item * 2).filter(item => item > 10);
};
"#
}

/// Sample Python code for testing
pub fn sample_python_code() -> &'static str {
    r#"
def greet(name):
    print(f"Hello, {name}")
    return f"greeting: {name}"

def calculate(a, b):
    result = a + b
    print(f"Result: {result}")
    return result

def process_data(data):
    return [item * 2 for item in data if item * 2 > 10]
"#
}

/// Sample TypeScript code for testing
pub fn sample_typescript_code() -> &'static str {
    r#"
interface User {
    name: string;
    age: number;
}

function greet(user: User): string {
    console.log(`Hello, ${user.name}`);
    return `greeting: ${user.name}`;
}

function calculate(a: number, b: number): number {
    const result = a + b;
    console.log(`Result: ${result}`);
    return result;
}

const processUsers = (users: User[]): User[] => {
    return users.filter(user => user.age > 18);
};
"#
}

/// Sample Rust code for testing
pub fn sample_rust_code() -> &'static str {
    r#"
fn greet(name: &str) -> String {
    println!("Hello, {}", name);
    format!("greeting: {}", name)
}

fn calculate(a: i32, b: i32) -> i32 {
    let result = a + b;
    println!("Result: {}", result);
    result
}

fn process_data(data: Vec<i32>) -> Vec<i32> {
    data.into_iter()
        .map(|item| item * 2)
        .filter(|&item| item > 10)
        .collect()
}
"#
}

/// Create a complete test project structure
pub fn create_test_project(temp_dir: &Path) {
    // Create directory structure
    fs::create_dir_all(temp_dir.join("src")).unwrap();
    fs::create_dir_all(temp_dir.join("tests")).unwrap();
    fs::create_dir_all(temp_dir.join("examples")).unwrap();

    // Create test files
    create_test_file(temp_dir, "src/main.js", sample_javascript_code());
    create_test_file(temp_dir, "src/utils.ts", sample_typescript_code());
    create_test_file(temp_dir, "src/helper.py", sample_python_code());
    create_test_file(temp_dir, "src/lib.rs", sample_rust_code());
    
    // Create some test files
    create_test_file(
        temp_dir,
        "tests/test1.js",
        "console.log('test1'); console.warn('warning1');",
    );
    create_test_file(
        temp_dir,
        "tests/test2.js", 
        "console.log('test2'); console.error('error2');",
    );
    
    // Create examples
    create_test_file(
        temp_dir,
        "examples/example.js",
        "function example() { console.log('example'); }",
    );
}

/// Assert that a search result contains expected matches
pub fn assert_search_matches(
    result: &ast_grep_mcp::types::SearchResult,
    expected_count: usize,
    expected_text_contains: Option<&str>,
) {
    assert_eq!(result.matches.len(), expected_count);
    
    if let Some(text) = expected_text_contains {
        assert!(result.matches.iter().any(|m| m.text.contains(text)));
    }
}

/// Assert that a replace result has expected properties
pub fn assert_replace_result(
    result: &ast_grep_mcp::types::ReplaceResult,
    expected_changes: usize,
    new_code_contains: Option<&str>,
) {
    assert_eq!(result.changes.len(), expected_changes);
    
    if let Some(text) = new_code_contains {
        assert!(result.new_code.contains(text));
    }
}

/// Common test patterns
pub mod patterns {
    pub const CONSOLE_LOG: &str = "console.log($VAR)";
    pub const CONSOLE_WARN: &str = "console.warn($VAR)";
    pub const CONSOLE_ERROR: &str = "console.error($VAR)";
    
    pub const FUNCTION_DECLARATION: &str = "function $NAME($PARAMS) { $BODY }";
    pub const ARROW_FUNCTION: &str = "($PARAMS) => { $BODY }";
    pub const CONST_ARROW_FUNCTION: &str = "const $NAME = ($PARAMS) => { $BODY }";
    
    pub const PYTHON_FUNCTION: &str = "def $NAME($PARAMS): $BODY";
    pub const PYTHON_PRINT: &str = "print($VAR)";
    
    pub const RUST_FUNCTION: &str = "fn $NAME($PARAMS) -> $TYPE { $BODY }";
    pub const RUST_PRINTLN: &str = "println!($VAR)";
}

/// Common replacement patterns
pub mod replacements {
    pub const CONSOLE_LOG_TO_WARN: (&str, &str) = ("console.log($VAR)", "console.warn($VAR)");
    pub const CONSOLE_LOG_TO_ERROR: (&str, &str) = ("console.log($VAR)", "console.error($VAR)");
    
    pub const FUNCTION_TO_ARROW: (&str, &str) = (
        "function $NAME($PARAMS) { $BODY }",
        "const $NAME = ($PARAMS) => { $BODY }",
    );
    
    pub const PYTHON_PRINT_TO_LOG: (&str, &str) = (
        "print($VAR)",
        "logging.info($VAR)",
    );
}