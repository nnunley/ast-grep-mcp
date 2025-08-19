//! # Refactoring Engine
//!
//! Core engine for executing refactoring operations using ast-grep.

use super::types::*;
use super::validation::ValidationEngine;
use super::capture_analysis::CaptureAnalysisEngine;
use crate::errors::ServiceError;
use crate::search::SearchService;
use crate::replace::ReplaceService;
use crate::types::{FileSearchParam, FileReplaceParam};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Core engine for executing refactorings
pub struct RefactoringEngine {
    search_service: Arc<SearchService>,
    replace_service: Arc<ReplaceService>,
    validation_engine: ValidationEngine,
    capture_analysis_engine: CaptureAnalysisEngine,
}

impl RefactoringEngine {
    /// Create a new refactoring engine
    pub fn new(
        search_service: Arc<SearchService>,
        replace_service: Arc<ReplaceService>,
    ) -> Self {
        Self {
            search_service,
            replace_service,
            validation_engine: ValidationEngine::new(),
            capture_analysis_engine: CaptureAnalysisEngine::new(),
        }
    }

    /// Execute a refactoring operation
    pub async fn execute(
        &self,
        definition: &RefactoringDefinition,
        request: &RefactoringRequest,
    ) -> Result<RefactoringResponse, ServiceError> {
        let options = request.options.as_ref();
        let language = options
            .and_then(|o| o.language.as_ref())
            .ok_or_else(|| ServiceError::Internal(
                "Language must be specified in options".to_string()
            ))?;

        // Determine the pattern to use
        let pattern = if let Some(ref pattern_example) = request.pattern_example {
            pattern_example.clone()
        } else {
            definition.pattern.r#match.clone()
        };

        // Build the search parameters
        let search_params = self.build_search_params(
            &pattern,
            language,
            &definition.pattern,
            options,
        )?;

        // Find matches
        info!("Searching for pattern matches");
        let search_results = self.search_service.file_search(search_params).await?;

        let total_matches: usize = search_results
            .matches
            .iter()
            .map(|f| f.matches.len())
            .sum();

        let files_affected: Vec<String> = search_results
            .matches
            .iter()
            .map(|f| f.file_path.clone())
            .collect();

        debug!("Found {} matches in {} files", total_matches, files_affected.len());

        // If no matches found, return early
        if total_matches == 0 {
            return Ok(RefactoringResponse {
                matches_found: 0,
                files_affected: vec![],
                changes_preview: None,
                applied: false,
                error: None,
                warnings: Some(vec!["No matches found for the specified pattern".to_string()]),
            });
        }

        // Check if we're in preview mode
        let preview_mode = options.map(|o| o.preview).unwrap_or(true);

        // Validate preconditions if defined
        if let Some(ref preconditions) = definition.preconditions {
            let validation_warnings = self.validation_engine.check_preconditions(
                preconditions,
                &search_results,
                definition,
                request,
            )?;

            if !validation_warnings.is_empty() {
                warn!("Precondition warnings: {:?}", validation_warnings);
                if !preview_mode {
                    return Ok(RefactoringResponse {
                        matches_found: total_matches,
                        files_affected,
                        changes_preview: None,
                        applied: false,
                        error: Some("Precondition validation failed".to_string()),
                        warnings: Some(validation_warnings),
                    });
                }
            }
        }

        // Build transformation with capture analysis
        let transformation = self.build_transformation(
            &definition.transform,
            request,
            &search_results.matches[0].matches[0].vars,
            &search_results.matches[0].matches[0],
            language,
        )?;

        // Apply or preview the refactoring
        if preview_mode {
            let preview = self.generate_preview(
                &pattern,
                &transformation,
                language,
                &files_affected,
                total_matches,
            ).await?;

            Ok(RefactoringResponse {
                matches_found: total_matches,
                files_affected,
                changes_preview: Some(preview),
                applied: false,
                error: None,
                warnings: None,
            })
        } else {
            // Apply the refactoring
            let replace_params = self.build_replace_params(
                &pattern,
                &transformation,
                language,
                &definition.pattern,
                options,
            )?;

            let _replace_results = self.replace_service.file_replace(replace_params).await?;

            Ok(RefactoringResponse {
                matches_found: total_matches,
                files_affected,
                changes_preview: None,
                applied: true,
                error: None,
                warnings: None,
            })
        }
    }

    /// Build search parameters from refactoring definition
    fn build_search_params(
        &self,
        pattern: &str,
        language: &str,
        pattern_def: &PatternDefinition,
        options: Option<&RefactoringOptions>,
    ) -> Result<FileSearchParam, ServiceError> {
        let mut params = FileSearchParam {
            pattern: pattern.to_string(),
            language: language.to_string(),
            path_pattern: options
                .and_then(|o| o.path_pattern.clone())
                .unwrap_or_else(|| "**/*".to_string()),
            max_results: options.map(|o| o.max_matches).unwrap_or(1000),
            ..Default::default()
        };

        // Apply constraints if any
        if let Some(ref constraints) = pattern_def.constraints {
            // Convert constraints to ast-grep context
            // This is a simplified version - full implementation would handle all constraint types
            params.context = Some(self.constraints_to_context(constraints)?);
        }

        Ok(params)
    }

    /// Convert pattern constraints to ast-grep context
    fn constraints_to_context(&self, constraints: &[PatternConstraint]) -> Result<String, ServiceError> {
        // This is a simplified implementation
        // A full implementation would generate proper YAML rule context
        let mut context = String::from("rule:\n  pattern: $PATTERN\n");
        
        for constraint in constraints {
            match &constraint.constraint_type {
                ConstraintType::Inside { context: ctx } => {
                    context.push_str(&format!("  inside:\n    kind: {}\n", ctx));
                }
                ConstraintType::Has { identifier } => {
                    context.push_str(&format!("  has:\n    kind: identifier\n    pattern: {}\n", identifier));
                }
                ConstraintType::Not { matches } => {
                    context.push_str(&format!("  not:\n    pattern: {}\n", matches));
                }
                _ => {
                    // Other constraint types would be handled here
                }
            }
        }

        Ok(context)
    }

    /// Build transformation string from definition and request
    fn build_transformation(
        &self,
        transform: &TransformDefinition,
        request: &RefactoringRequest,
        _captured_vars: &HashMap<String, String>,
        search_match: &crate::types::MatchResult,
        language: &str,
    ) -> Result<String, ServiceError> {
        let mut transformation = transform.replace.clone();

        // Perform capture analysis if this is an extraction refactoring
        if let Some(ref _extract) = transform.extract {
            debug!("Performing capture analysis for extraction refactoring");
            
            // Analyze the captured code fragment directly using the match result
            match self.capture_analysis_engine.analyze_capture(search_match, language, 3) {
                Ok(analysis) => {
                    info!("Capture analysis completed successfully");
                    
                    // Generate parameter suggestions
                    let suggested_params = self.capture_analysis_engine.suggest_parameters(&analysis);
                    let param_list: Vec<String> = suggested_params.iter().map(|p| p.name.clone()).collect();
                    let params_str = param_list.join(", ");
                    
                    // Generate return strategy
                    let return_strategy = self.capture_analysis_engine.suggest_return_strategy(&analysis);
                    
                    // Replace $PARAMS with suggested parameters
                    transformation = transformation.replace("$PARAMS", &params_str);
                    
                    // Replace $RETURN_VALUE based on return strategy
                    match return_strategy {
                        crate::refactoring::capture_analysis::ReturnStrategy::Single { expression, .. } => {
                            transformation = transformation.replace("$RETURN_VALUE", &expression);
                        },
                        crate::refactoring::capture_analysis::ReturnStrategy::Multiple { values } => {
                            let return_expr = format!("[{}]", values.join(", "));
                            transformation = transformation.replace("$RETURN_VALUE", &return_expr);
                        },
                        crate::refactoring::capture_analysis::ReturnStrategy::InPlace { modified_params } => {
                            // For in-place modifications, we might return the modified parameters
                            let return_expr = if modified_params.len() == 1 {
                                modified_params[0].clone()
                            } else {
                                format!("{{ {} }}", modified_params.join(", "))
                            };
                            transformation = transformation.replace("$RETURN_VALUE", &return_expr);
                        },
                        crate::refactoring::capture_analysis::ReturnStrategy::Void => {
                            // Remove return statement for void functions
                            transformation = transformation.replace("return $RETURN_VALUE;", "");
                            transformation = transformation.replace("$RETURN_VALUE", "");
                        },
                    }
                    
                    debug!("Applied capture analysis to transformation: {}", transformation);
                },
                Err(e) => {
                    warn!("Capture analysis failed: {}", e);
                    // Fall back to basic variable replacement without analysis
                }
            }
        }

        // Replace known variables from options
        if let Some(ref options) = request.options {
            if let Some(ref function_name) = options.function_name {
                transformation = transformation.replace("$FUNCTION_NAME", function_name);
            }
            if let Some(ref variable_name) = options.variable_name {
                transformation = transformation.replace("$VARIABLE_NAME", variable_name);
                transformation = transformation.replace("$VAR_NAME", variable_name);
            }
            if let Some(ref class_name) = options.class_name {
                transformation = transformation.replace("$CLASS_NAME", class_name);
                transformation = transformation.replace("$NEW_CLASS", class_name);
            }
            if let Some(ref new_name) = options.new_name {
                transformation = transformation.replace("$NEW_NAME", new_name);
            }
        }

        // Additional extraction handling is done above with capture analysis

        Ok(transformation)
    }

    /// Generate a preview of changes
    async fn generate_preview(
        &self,
        pattern: &str,
        transformation: &str,
        language: &str,
        files_affected: &[String],
        total_matches: usize,
    ) -> Result<ChangesPreview, ServiceError> {
        // Get a sample transformation
        let sample_params = FileReplaceParam {
            pattern: pattern.to_string(),
            replacement: transformation.to_string(),
            language: language.to_string(),
            path_pattern: files_affected.first().cloned().unwrap_or_default(),
            max_results: 1,
            dry_run: true,
            summary_only: false,
            include_samples: true,
            max_samples: 1,
            ..Default::default()
        };

        let sample_results = self.replace_service.file_replace(sample_params).await?;
        
        let example_transformation = if let Some(first_file) = sample_results.file_results.first() {
            if let Some(first_change) = first_file.changes.first() {
                format!(
                    "- {}\n+ {}",
                    first_change.old_text.trim(),
                    first_change.new_text.trim()
                )
            } else {
                "No changes to preview".to_string()
            }
        } else {
            "No changes to preview".to_string()
        };

        // Calculate total lines affected (simplified)
        let total_lines_affected = total_matches * 2; // Rough estimate

        Ok(ChangesPreview {
            total_lines_affected,
            example_transformation,
            change_summary: Some(HashMap::from([
                ("files_modified".to_string(), files_affected.len()),
                ("patterns_replaced".to_string(), total_matches),
            ])),
        })
    }

    /// Build replace parameters from refactoring definition
    fn build_replace_params(
        &self,
        pattern: &str,
        transformation: &str,
        language: &str,
        pattern_def: &PatternDefinition,
        options: Option<&RefactoringOptions>,
    ) -> Result<FileReplaceParam, ServiceError> {
        let mut params = FileReplaceParam {
            pattern: pattern.to_string(),
            replacement: transformation.to_string(),
            language: language.to_string(),
            path_pattern: options
                .and_then(|o| o.path_pattern.clone())
                .unwrap_or_else(|| "**/*".to_string()),
            max_results: options.map(|o| o.max_matches).unwrap_or(1000),
            dry_run: false,
            ..Default::default()
        };

        // Apply constraints if any
        if let Some(ref constraints) = pattern_def.constraints {
            params.context = Some(self.constraints_to_context(constraints)?);
        }

        Ok(params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServiceConfig;

    fn create_mock_search_service() -> Arc<SearchService> {
        Arc::new(SearchService::new(
            ServiceConfig::default(),
            Default::default(),
            Default::default(),
        ))
    }

    fn create_mock_replace_service() -> Arc<ReplaceService> {
        Arc::new(ReplaceService::new(
            ServiceConfig::default(),
            Default::default(),
            Default::default(),
        ))
    }

    #[allow(dead_code)]
    fn create_test_definition() -> RefactoringDefinition {
        RefactoringDefinition {
            id: "extract_variable".to_string(),
            name: "Extract Variable".to_string(),
            category: RefactoringCategory::ComposingMethods,
            description: "Extract expression into variable".to_string(),
            supported_languages: vec!["javascript".to_string()],
            complexity: RefactoringComplexity::Simple,
            pattern: PatternDefinition {
                r#match: "$EXPR".to_string(),
                constraints: None,
            },
            transform: TransformDefinition {
                replace: "$VAR_NAME".to_string(),
                extract: Some(ExtractDefinition {
                    r#type: ExtractType::Variable,
                    template: "const $VAR_NAME = $EXPR;".to_string(),
                    placement: PlacementStrategy::Before,
                }),
                scope_analysis: None,
                update_calls: None,
            },
            variables: None,
            preconditions: None,
            variants: None,
        }
    }

    #[test]
    fn test_engine_creation() {
        let search_service = create_mock_search_service();
        let replace_service = create_mock_replace_service();
        let _engine = RefactoringEngine::new(search_service, replace_service);
        
        // Engine should be created successfully
        // Note: Cannot access private config field, just test creation succeeds
    }

    #[test]
    fn test_build_transformation() {
        let search_service = create_mock_search_service();
        let replace_service = create_mock_replace_service();
        let engine = RefactoringEngine::new(search_service, replace_service);

        let transform = TransformDefinition {
            replace: "const $VARIABLE_NAME = $EXPR;".to_string(),
            extract: None,
            scope_analysis: None,
            update_calls: None,
        };

        let request = RefactoringRequest {
            refactoring_id: "extract_variable".to_string(),
            pattern_example: None,
            options: Some(RefactoringOptions {
                variable_name: Some("result".to_string()),
                ..Default::default()
            }),
        };

        let captured_vars = HashMap::new();
        let dummy_match = crate::types::MatchResult {
            text: "test".to_string(),
            start_line: 0,
            end_line: 0,
            start_col: 0,
            end_col: 0,
            vars: HashMap::new(),
            context_before: None,
            context_after: None,
        };
        let transformation = engine.build_transformation(&transform, &request, &captured_vars, &dummy_match, "javascript").unwrap();
        
        assert_eq!(transformation, "const result = $EXPR;");
    }

    #[test]
    fn test_constraints_to_context() {
        let search_service = create_mock_search_service();
        let replace_service = create_mock_replace_service();
        let engine = RefactoringEngine::new(search_service, replace_service);

        let constraints = vec![
            PatternConstraint {
                constraint_type: ConstraintType::Inside {
                    context: "function_declaration".to_string(),
                },
            },
        ];

        let context = engine.constraints_to_context(&constraints).unwrap();
        assert!(context.contains("inside:"));
        assert!(context.contains("function_declaration"));
    }
}