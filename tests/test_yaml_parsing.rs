#[cfg(test)]
mod yaml_parsing_tests {
    use ast_grep_mcp::refactoring::types::RefactoringDefinition;
    use std::fs;

    #[test]
    fn test_parse_extract_variable_yaml() {
        let content = fs::read_to_string("refactorings/extract_variable.yaml")
            .expect("Failed to read extract_variable.yaml");
        
        println!("YAML content length: {}", content.len());
        println!("First 500 chars: {}", &content[..500.min(content.len())]);
        
        let result: Result<RefactoringDefinition, _> = serde_yaml::from_str(&content);
        
        match result {
            Ok(definition) => {
                println!("Successfully parsed extract_variable:");
                println!("  ID: {}", definition.id);
                println!("  Name: {}", definition.name);
                println!("  Category: {:?}", definition.category);
            }
            Err(e) => {
                println!("Failed to parse YAML: {}", e);
                panic!("YAML parsing failed: {}", e);
            }
        }
    }
    
    #[test]
    fn test_parse_all_yaml_files() {
        let entries = fs::read_dir("refactorings").expect("Failed to read refactorings directory");
        
        for entry in entries {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                println!("Parsing: {:?}", path);
                
                let content = fs::read_to_string(&path)
                    .expect(&format!("Failed to read {:?}", path));
                
                let result: Result<RefactoringDefinition, _> = serde_yaml::from_str(&content);
                
                match result {
                    Ok(definition) => {
                        println!("  ✓ Successfully parsed: {}", definition.id);
                    }
                    Err(e) => {
                        println!("  ✗ Failed to parse {:?}: {}", path, e);
                        panic!("YAML parsing failed for {:?}: {}", path, e);
                    }
                }
            }
        }
    }
}