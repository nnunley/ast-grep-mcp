//! Learning system for ast-grep pattern education

pub mod discovery;
pub mod prompt_generation;
pub mod types;
pub mod validation;

pub use discovery::DiscoveryService;
pub use prompt_generation::{GeneratePromptParam, GeneratedPrompt, PromptConfig, PromptGenerator};
pub use types::*;
pub use validation::ValidationEngine;

/// Main learning service coordinator
#[derive(Clone)]
pub struct LearningService {
    pub validation: ValidationEngine,
    pub discovery: DiscoveryService,
    pub prompt_generator: PromptGenerator,
}

impl LearningService {
    pub fn new() -> Result<Self, crate::errors::ServiceError> {
        Ok(Self {
            validation: ValidationEngine::new(),
            discovery: DiscoveryService::new()?,
            prompt_generator: PromptGenerator::new(),
        })
    }

    pub async fn validate_pattern(
        &self,
        param: ValidatePatternParam,
    ) -> Result<ValidationResult, crate::errors::ServiceError> {
        self.validation.validate_pattern(param).await
    }

    pub async fn explore_patterns(
        &self,
        param: ExplorePatternParam,
    ) -> Result<PatternCatalog, crate::errors::ServiceError> {
        self.discovery.explore_patterns(param).await
    }

    /// Generate LLM prompts for enhanced learning assistance
    pub fn generate_prompt(
        &self,
        param: GeneratePromptParam,
    ) -> Result<GeneratedPrompt, crate::errors::ServiceError> {
        self.prompt_generator.generate_prompt(param)
    }

    /// Generate quick hint for validation results
    pub fn generate_quick_hint(
        &self,
        validation_result: &ValidationResult,
        pattern: &str,
    ) -> String {
        self.prompt_generator
            .generate_quick_hint(validation_result, pattern)
    }

    /// Generate debugging context for LLM assistance
    pub fn generate_debugging_context(&self, param: &GeneratePromptParam) -> String {
        self.prompt_generator.generate_debugging_context(param)
    }
}

impl Default for LearningService {
    fn default() -> Self {
        Self::new().expect("Failed to initialize learning service")
    }
}
