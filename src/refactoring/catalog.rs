//! # Refactoring Catalog
//!
//! Manages loading and accessing refactoring definitions from YAML files.

use super::types::*;
use crate::errors::ServiceError;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Manages a catalog of available refactorings
pub struct RefactoringCatalog {
    /// Map of refactoring ID to definition
    refactorings: HashMap<String, RefactoringDefinition>,
    /// Path to refactorings directory
    catalog_path: PathBuf,
}

impl RefactoringCatalog {
    /// Create a new catalog from a directory path
    pub fn new(catalog_path: impl Into<PathBuf>) -> Self {
        Self {
            refactorings: HashMap::new(),
            catalog_path: catalog_path.into(),
        }
    }

    /// Load the default catalog from the refactorings directory
    pub fn load_default() -> Result<Self, Box<dyn std::error::Error>> {
        let catalog_path = PathBuf::from("refactorings");
        let mut catalog = Self::new(catalog_path);
        catalog.load_all()?;
        Ok(catalog)
    }

    /// Load all refactoring definitions from the catalog directory
    pub fn load_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.catalog_path.exists() {
            return Err(format!(
                "Refactorings directory not found: {}",
                self.catalog_path.display()
            )
            .into());
        }

        let entries = fs::read_dir(&self.catalog_path)?;
        let mut loaded_count = 0;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                match self.load_refactoring_file(&path) {
                    Ok(()) => loaded_count += 1,
                    Err(e) => warn!("Failed to load refactoring from {:?}: {}", path, e),
                }
            }
        }

        debug!("Loaded {} refactoring definitions", loaded_count);
        Ok(())
    }

    /// Load a single refactoring definition from a YAML file
    pub fn load_refactoring_file(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let definition: RefactoringDefinition = serde_yaml::from_str(&content)?;

        debug!(
            "Loaded refactoring '{}' from {:?}",
            definition.id,
            path.file_name()
        );

        self.refactorings.insert(definition.id.clone(), definition);
        Ok(())
    }

    /// Get a refactoring definition by ID
    pub fn get(&self, id: &str) -> Option<&RefactoringDefinition> {
        self.refactorings.get(id)
    }

    /// Get all available refactoring IDs
    pub fn list_ids(&self) -> Vec<String> {
        self.refactorings.keys().cloned().collect()
    }

    /// Get refactorings by category
    pub fn by_category(&self, category: RefactoringCategory) -> Vec<&RefactoringDefinition> {
        self.refactorings
            .values()
            .filter(|r| r.category == category)
            .collect()
    }

    /// Get refactorings that support a specific language
    pub fn for_language(&self, language: &str) -> Vec<&RefactoringDefinition> {
        self.refactorings
            .values()
            .filter(|r| r.supported_languages.iter().any(|l| l == language))
            .collect()
    }

    /// Validate that a refactoring can be applied with given options
    pub fn validate_request(
        &self,
        request: &RefactoringRequest,
    ) -> Result<&RefactoringDefinition, ServiceError> {
        let definition = self
            .get(&request.refactoring_id)
            .ok_or_else(|| ServiceError::Internal(
                format!("Unknown refactoring ID: {}", request.refactoring_id)
            ))?;

        // Validate language support if specified
        if let Some(ref options) = request.options {
            if let Some(ref language) = options.language {
                if !definition.supported_languages.contains(language) {
                    return Err(ServiceError::Internal(
                        format!(
                            "Refactoring '{}' does not support language: {}",
                            request.refactoring_id, language
                        )
                    ));
                }
            }
        }

        // Validate required options based on refactoring type
        if let Some(ref options) = request.options {
            match request.refactoring_id.as_str() {
                "extract_method" | "extract_function" => {
                    if options.function_name.is_none() {
                        return Err(ServiceError::Internal(
                            "extract_method requires function_name in options".to_string()
                        ));
                    }
                }
                "extract_variable" => {
                    if options.variable_name.is_none() {
                        return Err(ServiceError::Internal(
                            "extract_variable requires variable_name in options"
                                .to_string()
                        ));
                    }
                }
                "rename_symbol" => {
                    if options.new_name.is_none() {
                        return Err(ServiceError::Internal(
                            "rename_symbol requires new_name in options".to_string()
                        ));
                    }
                }
                "extract_class" => {
                    if options.class_name.is_none() {
                        return Err(ServiceError::Internal(
                            "extract_class requires class_name in options".to_string()
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(definition)
    }

    /// Get a summary of the catalog for display
    pub fn summary(&self) -> String {
        let mut summary = format!("Refactoring Catalog: {} definitions\n", self.refactorings.len());
        
        // Group by category
        let mut by_category: HashMap<RefactoringCategory, Vec<&str>> = HashMap::new();
        for (id, def) in &self.refactorings {
            by_category
                .entry(def.category)
                .or_default()
                .push(id);
        }

        for (category, ids) in by_category {
            summary.push_str(&format!("\n{:?}:\n", category));
            for id in ids {
                if let Some(def) = self.refactorings.get(id) {
                    summary.push_str(&format!("  - {} ({})\n", id, def.name));
                }
            }
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_refactoring_yaml() -> &'static str {
        r#"
id: test_extract_var
name: Test Extract Variable
category: composing_methods
description: Test refactoring
supported_languages: [javascript, typescript]
complexity: simple
pattern:
  match: |
    $EXPR
transform:
  replace: |
    $VAR_NAME
  extract:
    type: variable
    template: |
      const $VAR_NAME = $EXPR;
    placement: before
"#
    }

    #[test]
    fn test_catalog_creation() {
        let catalog = RefactoringCatalog::new("test/path");
        assert_eq!(catalog.list_ids().len(), 0);
    }

    #[test]
    fn test_load_refactoring_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_refactoring.yaml");
        fs::write(&file_path, create_test_refactoring_yaml()).unwrap();

        let mut catalog = RefactoringCatalog::new(temp_dir.path());
        catalog.load_refactoring_file(&file_path).unwrap();

        assert_eq!(catalog.list_ids().len(), 1);
        assert!(catalog.get("test_extract_var").is_some());
    }

    #[test]
    fn test_get_by_category() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_refactoring.yaml");
        fs::write(&file_path, create_test_refactoring_yaml()).unwrap();

        let mut catalog = RefactoringCatalog::new(temp_dir.path());
        catalog.load_refactoring_file(&file_path).unwrap();

        let composing_methods = catalog.by_category(RefactoringCategory::ComposingMethods);
        assert_eq!(composing_methods.len(), 1);
    }

    #[test]
    fn test_for_language() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_refactoring.yaml");
        fs::write(&file_path, create_test_refactoring_yaml()).unwrap();

        let mut catalog = RefactoringCatalog::new(temp_dir.path());
        catalog.load_refactoring_file(&file_path).unwrap();

        let js_refactorings = catalog.for_language("javascript");
        assert_eq!(js_refactorings.len(), 1);

        let rust_refactorings = catalog.for_language("rust");
        assert_eq!(rust_refactorings.len(), 0);
    }

    #[test]
    fn test_validate_request() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_refactoring.yaml");
        fs::write(&file_path, create_test_refactoring_yaml()).unwrap();

        let mut catalog = RefactoringCatalog::new(temp_dir.path());
        catalog.load_refactoring_file(&file_path).unwrap();

        // Valid request
        let request = RefactoringRequest {
            refactoring_id: "test_extract_var".to_string(),
            pattern_example: None,
            options: Some(RefactoringOptions {
                language: Some("javascript".to_string()),
                ..Default::default()
            }),
        };

        assert!(catalog.validate_request(&request).is_ok());

        // Invalid language
        let invalid_request = RefactoringRequest {
            refactoring_id: "test_extract_var".to_string(),
            pattern_example: None,
            options: Some(RefactoringOptions {
                language: Some("rust".to_string()),
                ..Default::default()
            }),
        };

        assert!(catalog.validate_request(&invalid_request).is_err());

        // Unknown refactoring
        let unknown_request = RefactoringRequest {
            refactoring_id: "unknown_refactoring".to_string(),
            pattern_example: None,
            options: None,
        };

        assert!(catalog.validate_request(&unknown_request).is_err());
    }

    #[test]
    fn test_catalog_summary() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_refactoring.yaml");
        fs::write(&file_path, create_test_refactoring_yaml()).unwrap();

        let mut catalog = RefactoringCatalog::new(temp_dir.path());
        catalog.load_refactoring_file(&file_path).unwrap();

        let summary = catalog.summary();
        assert!(summary.contains("Refactoring Catalog: 1 definitions"));
        assert!(summary.contains("ComposingMethods"));
        assert!(summary.contains("test_extract_var"));
    }
}