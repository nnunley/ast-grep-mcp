use std::fs;
use ast_grep_mcp::refactoring::catalog::RefactoringCatalog;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing refactoring catalog loading...");
    
    let mut catalog = RefactoringCatalog::new("refactorings");
    catalog.load_all()?;
    
    let ids = catalog.list_ids();
    println!("Found {} refactorings:", ids.len());
    for id in &ids {
        if let Some(def) = catalog.get(id) {
            println!("  - {} ({}): {}", id, def.name, def.description);
        }
    }
    
    // Test a specific refactoring
    if let Some(extract_var) = catalog.get("extract_variable") {
        println!("\nTesting extract_variable:");
        println!("  Category: {:?}", extract_var.category);
        println!("  Languages: {:?}", extract_var.supported_languages);
        println!("  Pattern: {}", extract_var.pattern.r#match);
    }
    
    println!("\nCatalog summary:");
    println!("{}", catalog.summary());
    
    Ok(())
}