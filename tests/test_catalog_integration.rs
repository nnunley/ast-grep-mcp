#[cfg(test)]
mod catalog_integration_tests {
    use ast_grep_mcp::refactoring::catalog::RefactoringCatalog;

    #[test]
    fn test_load_all_refactorings() {
        let mut catalog = RefactoringCatalog::new("refactorings");
        
        // Test individual file loading to find which one fails
        let test_files = [
            "extract_variable.yaml",
            "rename_symbol.yaml", 
            "extract_method.yaml",
            "inline_variable.yaml",
            "replace_conditional_with_guard.yaml",
            "extract_class.yaml",
            "replace_loop_with_pipeline.yaml",
            "consolidate_duplicate_conditional.yaml",
            "replace_magic_number.yaml",
            "introduce_parameter_object.yaml"
        ];
        
        for file in &test_files {
            let path = std::path::Path::new("refactorings").join(file);
            match catalog.load_refactoring_file(&path) {
                Ok(()) => println!("✓ Successfully loaded {}", file),
                Err(e) => println!("✗ Failed to load {}: {}", file, e),
            }
        }
        
        let result = catalog.load_all();
        
        // Should successfully load all refactorings
        assert!(result.is_ok(), "Failed to load refactorings: {:?}", result.err());
        
        let ids = catalog.list_ids();
        println!("Loaded {} refactorings: {:?}", ids.len(), ids);
        
        // We should have our 10 refactorings
        assert!(ids.len() >= 10, "Expected at least 10 refactorings, got {}", ids.len());
        
        // Check for specific refactorings
        let expected_refactorings = [
            "extract_variable",
            "rename_symbol", 
            "extract_method",
            "inline_variable",
            "replace_conditional_with_guard",
            "extract_class",
            "replace_loop_with_pipeline",
            "consolidate_duplicate_conditional",
            "replace_magic_number",
            "introduce_parameter_object"
        ];
        
        for expected in &expected_refactorings {
            assert!(ids.contains(&expected.to_string()), 
                   "Missing expected refactoring: {}", expected);
        }
    }
    
    #[test]
    fn test_refactoring_definitions() {
        let mut catalog = RefactoringCatalog::new("refactorings");
        catalog.load_all().expect("Failed to load catalog");
        
        // Test extract_variable definition
        let extract_var = catalog.get("extract_variable").expect("extract_variable not found");
        assert_eq!(extract_var.name, "Extract Variable");
        assert_eq!(extract_var.complexity, ast_grep_mcp::refactoring::types::RefactoringComplexity::Simple);
        assert!(extract_var.supported_languages.contains(&"javascript".to_string()));
        
        // Test rename_symbol definition  
        let rename_symbol = catalog.get("rename_symbol").expect("rename_symbol not found");
        assert_eq!(rename_symbol.name, "Rename Symbol");
        assert_eq!(rename_symbol.complexity, ast_grep_mcp::refactoring::types::RefactoringComplexity::Moderate);
        
        // Test extract_method definition
        let extract_method = catalog.get("extract_method").expect("extract_method not found");
        assert_eq!(extract_method.name, "Extract Method");
        assert!(extract_method.variants.is_some());
        
        println!("Catalog summary:");
        println!("{}", catalog.summary());
    }
    
    #[test]
    fn test_refactoring_by_category() {
        let mut catalog = RefactoringCatalog::new("refactorings");
        catalog.load_all().expect("Failed to load catalog");
        
        use ast_grep_mcp::refactoring::types::RefactoringCategory;
        
        let composing_methods = catalog.by_category(RefactoringCategory::ComposingMethods);
        let organizing_data = catalog.by_category(RefactoringCategory::OrganizingData);
        let organizing_code = catalog.by_category(RefactoringCategory::OrganizingCode);
        let simplifying_conditionals = catalog.by_category(RefactoringCategory::SimplifyingConditionals);
        
        println!("Composing Methods: {}", composing_methods.len());
        println!("Organizing Data: {}", organizing_data.len());
        println!("Organizing Code: {}", organizing_code.len());
        println!("Simplifying Conditionals: {}", simplifying_conditionals.len());
        
        // We should have refactorings in each category
        assert!(!composing_methods.is_empty());
        assert!(!organizing_data.is_empty());
        assert!(!organizing_code.is_empty());
        assert!(!simplifying_conditionals.is_empty());
    }
    
    #[test]
    fn test_refactoring_by_language() {
        let mut catalog = RefactoringCatalog::new("refactorings");
        catalog.load_all().expect("Failed to load catalog");
        
        let js_refactorings = catalog.for_language("javascript");
        let ts_refactorings = catalog.for_language("typescript");
        let python_refactorings = catalog.for_language("python");
        let rust_refactorings = catalog.for_language("rust");
        
        println!("JavaScript: {}", js_refactorings.len());
        println!("TypeScript: {}", ts_refactorings.len());
        println!("Python: {}", python_refactorings.len());
        println!("Rust: {}", rust_refactorings.len());
        
        // All languages should have some refactorings
        assert!(!js_refactorings.is_empty());
        assert!(!ts_refactorings.is_empty());
        assert!(!python_refactorings.is_empty());
        assert!(!rust_refactorings.is_empty());
    }
}