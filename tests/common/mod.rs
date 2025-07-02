// Common test utilities
use tempfile::TempDir;
use std::fs;

pub fn setup_test_files() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test JavaScript file
    let js_content = r#"
function greet() {
    console.log("Hello, world!");
    console.log("Welcome!");
}

function goodbye() {
    alert("Goodbye!");
}
"#;
    fs::write(temp_dir.path().join("test.js"), js_content).unwrap();
    
    // Create test Rust file  
    let rust_content = r#"
fn main() {
    println!("Hello, Rust!");
    println!("Testing ast-grep");
}
"#;
    fs::write(temp_dir.path().join("test.rs"), rust_content).unwrap();
    
    temp_dir
}