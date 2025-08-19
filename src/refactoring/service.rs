//! # Refactoring Service
//!
//! Main service that orchestrates refactoring operations and handles MCP tool requests.

use super::catalog::RefactoringCatalog;
use super::engine::RefactoringEngine;
use super::types::*;
use super::validation::ValidationEngine;
use crate::errors::ServiceError;
use crate::search::SearchService;
use crate::replace::ReplaceService;
use std::sync::{Arc, RwLock};
use tracing::{debug, info, error};
use serde::{Serialize, Deserialize};

/// Main refactoring service that handles MCP tool requests
pub struct RefactoringService {
    catalog: Arc<RwLock<RefactoringCatalog>>,
    engine: Arc<RefactoringEngine>,
    validation_engine: Arc<ValidationEngine>,
}

impl RefactoringService {
    /// Create a new refactoring service
    pub fn new(
        search_service: Arc<SearchService>,
        replace_service: Arc<ReplaceService>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Load the default catalog
        let catalog = RefactoringCatalog::load_default()?;
        let catalog = Arc::new(RwLock::new(catalog));
        
        let engine = Arc::new(RefactoringEngine::new(
            search_service,
            replace_service,
        ));
        
        let validation_engine = Arc::new(ValidationEngine::new());

        Ok(Self {
            catalog,
            engine,
            validation_engine,
        })
    }

    /// Create a service with a custom catalog path
    pub fn with_catalog_path(
        catalog_path: impl Into<std::path::PathBuf>,
        search_service: Arc<SearchService>,
        replace_service: Arc<ReplaceService>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut catalog = RefactoringCatalog::new(catalog_path);
        catalog.load_all()?;
        let catalog = Arc::new(RwLock::new(catalog));
        
        let engine = Arc::new(RefactoringEngine::new(
            search_service,
            replace_service,
        ));
        
        let validation_engine = Arc::new(ValidationEngine::new());

        Ok(Self {
            catalog,
            engine,
            validation_engine,
        })
    }

    /// Apply a refactoring based on the request
    pub async fn refactor(
        &self,
        request: RefactoringRequest,
    ) -> Result<RefactoringResponse, ServiceError> {
        info!("Processing refactoring request: {}", request.refactoring_id);

        // Get and validate the refactoring definition
        let definition = {
            let catalog = self.catalog.read().map_err(|_| ServiceError::Internal(
                "Failed to acquire catalog lock".to_string()
            ))?;
            
            catalog.validate_request(&request)?.clone()
        };

        debug!("Found refactoring definition: {}", definition.name);

        // Execute the refactoring
        match self.engine.execute(&definition, &request).await {
            Ok(response) => {
                info!(
                    "Refactoring completed: {} matches in {} files",
                    response.matches_found,
                    response.files_affected.len()
                );
                Ok(response)
            }
            Err(e) => {
                error!("Refactoring failed: {}", e);
                Err(e)
            }
        }
    }

    /// Validate a refactoring pattern against test code
    pub async fn validate_refactoring(
        &self,
        request: ValidateRefactoringRequest,
    ) -> Result<ValidateRefactoringResponse, ServiceError> {
        info!("Validating refactoring pattern: {}", request.refactoring_id);

        // Get the refactoring definition
        let definition = {
            let catalog = self.catalog.read().map_err(|_| ServiceError::Internal(
                "Failed to acquire catalog lock".to_string()
            ))?;
            
            catalog
                .get(&request.refactoring_id)
                .ok_or_else(|| ServiceError::Internal(
                    format!("Unknown refactoring ID: {}", request.refactoring_id)
                ))?
                .clone()
        };

        // Validate the pattern
        let response = self.validation_engine.validate_pattern(
            &definition,
            &request.test_code,
            &request.language,
            request.custom_pattern.as_deref(),
        );

        debug!(
            "Pattern validation complete: {} matches found",
            response.matches.len()
        );

        Ok(response)
    }

    /// List all available refactorings
    pub async fn list_refactorings(&self) -> Result<Vec<RefactoringInfo>, ServiceError> {
        let catalog = self.catalog.read().map_err(|_| ServiceError::Internal(
            "Failed to acquire catalog lock".to_string()
        ))?;

        let mut refactorings = Vec::new();
        
        for id in catalog.list_ids() {
            if let Some(definition) = catalog.get(&id) {
                refactorings.push(RefactoringInfo {
                    id: definition.id.clone(),
                    name: definition.name.clone(),
                    category: format!("{:?}", definition.category),
                    description: definition.description.clone(),
                    supported_languages: definition.supported_languages.clone(),
                    complexity: format!("{:?}", definition.complexity),
                });
            }
        }

        refactorings.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));

        Ok(refactorings)
    }

    /// Get detailed information about a specific refactoring
    pub async fn get_refactoring_info(
        &self,
        refactoring_id: &str,
    ) -> Result<RefactoringDetails, ServiceError> {
        let catalog = self.catalog.read().map_err(|_| ServiceError::Internal(
            "Failed to acquire catalog lock".to_string()
        ))?;

        let definition = catalog
            .get(refactoring_id)
            .ok_or_else(|| ServiceError::Internal(
                format!("Unknown refactoring ID: {}", refactoring_id)
            ))?;

        // Extract examples if available
        let examples = vec![]; // Would be extracted from YAML in full implementation

        // List required options
        let mut required_options = vec![];
        match refactoring_id {
            "extract_method" | "extract_function" => {
                required_options.push("function_name".to_string());
            }
            "extract_variable" => {
                required_options.push("variable_name".to_string());
            }
            "rename_symbol" => {
                required_options.push("new_name".to_string());
            }
            "extract_class" => {
                required_options.push("class_name".to_string());
            }
            _ => {}
        }

        // Get pattern variants
        let variants = definition
            .variants
            .as_ref()
            .map(|v| v.iter().map(|var| var.id.clone()).collect())
            .unwrap_or_default();

        Ok(RefactoringDetails {
            id: definition.id.clone(),
            name: definition.name.clone(),
            category: format!("{:?}", definition.category),
            description: definition.description.clone(),
            supported_languages: definition.supported_languages.clone(),
            complexity: format!("{:?}", definition.complexity),
            pattern: definition.pattern.r#match.clone(),
            transformation: definition.transform.replace.clone(),
            required_options,
            preconditions: definition
                .preconditions
                .as_ref()
                .map(|p| p.iter().map(|pre| format!("{:?}", pre)).collect())
                .unwrap_or_default(),
            examples,
            variants,
        })
    }

    /// Reload the refactoring catalog
    pub async fn reload_catalog(&self) -> Result<(), ServiceError> {
        let mut catalog = self.catalog.write().map_err(|_| ServiceError::Internal(
            "Failed to acquire catalog write lock".to_string()
        ))?;

        catalog.load_all().map_err(|e| ServiceError::Internal(
            format!("Failed to reload catalog: {}", e)
        ))?;

        info!("Refactoring catalog reloaded successfully");
        Ok(())
    }
}

/// Basic information about a refactoring
#[derive(Debug, Serialize, Deserialize)]
pub struct RefactoringInfo {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub supported_languages: Vec<String>,
    pub complexity: String,
}

/// Detailed information about a refactoring
#[derive(Debug, Serialize, Deserialize)]
pub struct RefactoringDetails {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub supported_languages: Vec<String>,
    pub complexity: String,
    pub pattern: String,
    pub transformation: String,
    pub required_options: Vec<String>,
    pub preconditions: Vec<String>,
    pub examples: Vec<RefactoringExample>,
    pub variants: Vec<String>,
}

/// Example of a refactoring transformation
#[derive(Debug, Serialize, Deserialize)]
pub struct RefactoringExample {
    pub description: String,
    pub before: String,
    pub after: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServiceConfig;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_services() -> (Arc<SearchService>, Arc<ReplaceService>) {
        let config = ServiceConfig::default();
        let search_service = Arc::new(SearchService::new(
            config.clone(),
            Default::default(),
            Default::default(),
        ));
        let replace_service = Arc::new(ReplaceService::new(
            config,
            Default::default(),
            Default::default(),
        ));
        (search_service, replace_service)
    }

    fn create_test_catalog_yaml() -> &'static str {
        r#"
id: test_rename
name: Test Rename
category: organizing_code
description: Test rename refactoring
supported_languages: [javascript]
complexity: simple
pattern:
  match: |
    $OLD_NAME
transform:
  replace: |
    $NEW_NAME
"#
    }

    #[tokio::test]
    async fn test_service_creation() {
        let (search_service, replace_service) = create_test_services();
        
        // Create temp directory with test refactoring
        let temp_dir = TempDir::new().unwrap();
        let refactorings_dir = temp_dir.path().join("refactorings");
        fs::create_dir(&refactorings_dir).unwrap();
        fs::write(
            refactorings_dir.join("test.yaml"),
            create_test_catalog_yaml(),
        ).unwrap();

        let service = RefactoringService::with_catalog_path(
            refactorings_dir,
            search_service,
            replace_service,
        );

        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_list_refactorings() {
        let (search_service, replace_service) = create_test_services();
        
        // Create temp directory with test refactoring
        let temp_dir = TempDir::new().unwrap();
        let refactorings_dir = temp_dir.path().join("refactorings");
        fs::create_dir(&refactorings_dir).unwrap();
        fs::write(
            refactorings_dir.join("test.yaml"),
            create_test_catalog_yaml(),
        ).unwrap();

        let service = RefactoringService::with_catalog_path(
            refactorings_dir,
            search_service,
            replace_service,
        ).unwrap();

        let refactorings = service.list_refactorings().await.unwrap();
        assert_eq!(refactorings.len(), 1);
        assert_eq!(refactorings[0].id, "test_rename");
    }

    #[tokio::test]
    async fn test_get_refactoring_info() {
        let (search_service, replace_service) = create_test_services();
        
        // Create temp directory with test refactoring
        let temp_dir = TempDir::new().unwrap();
        let refactorings_dir = temp_dir.path().join("refactorings");
        fs::create_dir(&refactorings_dir).unwrap();
        fs::write(
            refactorings_dir.join("test.yaml"),
            create_test_catalog_yaml(),
        ).unwrap();

        let service = RefactoringService::with_catalog_path(
            refactorings_dir,
            search_service,
            replace_service,
        ).unwrap();

        let info = service.get_refactoring_info("test_rename").await.unwrap();
        assert_eq!(info.id, "test_rename");
        assert_eq!(info.pattern, "$OLD_NAME");
        assert_eq!(info.transformation, "$NEW_NAME");
    }

    #[tokio::test]
    async fn test_validate_refactoring() {
        let (search_service, replace_service) = create_test_services();
        
        // Create temp directory with test refactoring
        let temp_dir = TempDir::new().unwrap();
        let refactorings_dir = temp_dir.path().join("refactorings");
        fs::create_dir(&refactorings_dir).unwrap();
        fs::write(
            refactorings_dir.join("test.yaml"),
            create_test_catalog_yaml(),
        ).unwrap();

        let service = RefactoringService::with_catalog_path(
            refactorings_dir,
            search_service,
            replace_service,
        ).unwrap();

        let request = ValidateRefactoringRequest {
            refactoring_id: "test_rename".to_string(),
            test_code: "const oldName = 1; oldName + 2;".to_string(),
            language: "javascript".to_string(),
            custom_pattern: Some("oldName".to_string()),
        };

        let response = service.validate_refactoring(request).await.unwrap();
        assert!(response.is_valid);
        assert_eq!(response.matches.len(), 2);
    }
}