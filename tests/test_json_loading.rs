//! Test JSON pattern loading

use ast_grep_mcp::learning::discovery::LanguagePatternData;

#[test]
fn test_javascript_json_parsing() {
    let js_data = include_str!("../src/data/patterns/javascript.json");

    let result: Result<LanguagePatternData, serde_json::Error> = serde_json::from_str(js_data);

    match &result {
        Ok(data) => {
            println!(
                "JavaScript patterns loaded successfully: {} patterns",
                data.patterns.len()
            );
            println!("Language: {}", data.language);
            println!("Progressions: {}", data.learning_progressions.len());
            println!("Categories: {}", data.categories.len());
        }
        Err(e) => {
            println!("Failed to parse JavaScript JSON: {e}");
        }
    }

    assert!(result.is_ok(), "JavaScript JSON should parse successfully");

    let data = result.unwrap();
    assert!(!data.patterns.is_empty(), "Should have patterns");
    assert_eq!(data.language, "javascript");
}

#[test]
fn test_rust_json_parsing() {
    let rust_data = include_str!("../src/data/patterns/rust.json");

    let result: Result<LanguagePatternData, serde_json::Error> = serde_json::from_str(rust_data);

    match &result {
        Ok(data) => {
            println!(
                "Rust patterns loaded successfully: {} patterns",
                data.patterns.len()
            );
        }
        Err(e) => {
            println!("Failed to parse Rust JSON: {e}");
        }
    }

    assert!(result.is_ok(), "Rust JSON should parse successfully");
}

#[test]
fn test_python_json_parsing() {
    let python_data = include_str!("../src/data/patterns/python.json");

    let result: Result<LanguagePatternData, serde_json::Error> = serde_json::from_str(python_data);

    match &result {
        Ok(data) => {
            println!(
                "Python patterns loaded successfully: {} patterns",
                data.patterns.len()
            );
        }
        Err(e) => {
            println!("Failed to parse Python JSON: {e}");
        }
    }

    assert!(result.is_ok(), "Python JSON should parse successfully");
}
