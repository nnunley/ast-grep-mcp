//! Test automatic language injection for HTML/JS/CSS

use ast_grep_mcp::{
    FileSearchParam, SearchParam, ast_grep_service::AstGrepService, config::ServiceConfig,
};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_automatic_js_in_html_search() {
    let service = AstGrepService::new();

    let html_code = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Test</title>
</head>
<body>
    <h1>Hello</h1>
    <script>
        function greet(name) {
            console.log("Hello, " + name);
        }
        greet("World");
    </script>
</body>
</html>
"#;

    let param = SearchParam {
        code: html_code.to_string(),
        pattern: "console.log($MSG)".to_string(),
        language: "javascript".to_string(), // Searching for JS pattern
        selector: None,
        context: None,
        strictness: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let result = service.search(param).await.unwrap();

    // Should find the console.log in the script tag
    assert_eq!(result.matches.len(), 1);
    assert_eq!(
        result.matches[0].vars.get("MSG").unwrap(),
        r#""Hello, " + name"#
    );
}

#[tokio::test]
async fn test_automatic_css_in_html_search() {
    let service = AstGrepService::new();

    let html_code = r#"
<!DOCTYPE html>
<html>
<head>
    <style>
        .container {
            display: flex;
            color: red;
        }
        .item {
            color: blue;
        }
    </style>
</head>
<body>
    <div class="container">Test</div>
</body>
</html>
"#;

    let param = SearchParam {
        code: html_code.to_string(),
        pattern: "color: $COLOR".to_string(),
        language: "css".to_string(), // Searching for CSS pattern
        selector: None,
        context: None,
        strictness: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let result = service.search(param).await.unwrap();

    // Should find both color declarations
    assert_eq!(result.matches.len(), 2);
    let colors: Vec<&str> = result
        .matches
        .iter()
        .map(|m| m.vars.get("COLOR").unwrap().as_str())
        .collect();
    assert!(colors.contains(&"red"));
    assert!(colors.contains(&"blue"));
}

#[tokio::test]
async fn test_automatic_js_in_html_file() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("index.html");

    let html_content = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Test Page</title>
</head>
<body>
    <button onclick="handleClick()">Click me</button>
    <script>
        function handleClick() {
            alert('Button clicked!');
        }

        function init() {
            console.log('Page loaded');
        }

        window.onload = init;
    </script>
</body>
</html>
"#;

    fs::write(&test_file, html_content).unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let param = FileSearchParam {
        path_pattern: test_file.to_string_lossy().to_string(),
        pattern: "function $NAME($PARAMS) { $BODY }".to_string(),
        language: "javascript".to_string(), // JS pattern in HTML file
        max_results: 10,
        cursor: None,
        max_file_size: 1024 * 1024,
        selector: None,
        context: None,
        strictness: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let result = service.file_search(param).await.unwrap();

    // Should find both functions in the script tag
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 2);

    let function_names: Vec<&str> = result.matches[0]
        .matches
        .iter()
        .map(|m| m.vars.get("NAME").unwrap().as_str())
        .collect();
    assert!(function_names.contains(&"handleClick"));
    assert!(function_names.contains(&"init"));
}

#[tokio::test]
async fn test_no_injection_for_js_file() {
    // Test that JS files are searched normally without injection
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("app.js");

    let js_content = r#"
function greet(name) {
    console.log("Hello, " + name);
}

greet("World");
"#;

    fs::write(&test_file, js_content).unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let param = FileSearchParam {
        path_pattern: test_file.to_string_lossy().to_string(),
        pattern: "console.log($MSG)".to_string(),
        language: "javascript".to_string(),
        max_results: 10,
        cursor: None,
        max_file_size: 1024 * 1024,
        selector: None,
        context: None,
        strictness: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let result = service.file_search(param).await.unwrap();

    // Should find the console.log normally
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].matches.len(), 1);
    assert_eq!(
        result.matches[0].matches[0].vars.get("MSG").unwrap(),
        r#""Hello, " + name"#
    );
}

#[tokio::test]
async fn test_vue_component_js_injection() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("App.vue");

    let vue_content = r#"
<template>
    <div>
        <h1>{{ title }}</h1>
        <button @click="increment">Count: {{ count }}</button>
    </div>
</template>

<script>
export default {
    data() {
        return {
            title: 'Vue App',
            count: 0
        }
    },
    methods: {
        increment() {
            this.count++
        }
    }
}
</script>

<style>
h1 {
    color: blue;
}
</style>
"#;

    fs::write(&test_file, vue_content).unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };
    let service = AstGrepService::with_config(config);

    let param = FileSearchParam {
        path_pattern: test_file.to_string_lossy().to_string(),
        pattern: "methods: { $METHODS }".to_string(),
        language: "javascript".to_string(),
        max_results: 10,
        cursor: None,
        max_file_size: 1024 * 1024,
        selector: None,
        context: None,
        strictness: None,
        context_before: None,
        context_after: None,
        context_lines: None,
    };

    let result = service.file_search(param).await.unwrap();

    // Should find the methods object in the Vue component
    assert_eq!(result.matches.len(), 1);
    assert!(result.matches[0].matches[0].text.contains("increment"));
}
