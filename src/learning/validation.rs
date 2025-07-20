//! Pattern validation with learning guidance

use super::prompt_generation::{
    GeneratePromptParam, InteractionStyle, LearningLevel, LlmType, PromptConfig, PromptGenerator,
};
use super::types::*;
use crate::errors::ServiceError;
use crate::types::MatchResult;
use ast_grep_core::{AstGrep, Pattern};
use ast_grep_language::SupportLang;
use std::str::FromStr;

#[derive(Clone)]
pub struct ValidationEngine {
    prompt_generator: PromptGenerator,
}

impl ValidationEngine {
    pub fn new() -> Self {
        Self {
            prompt_generator: PromptGenerator::new(),
        }
    }

    pub async fn validate_pattern(
        &self,
        param: ValidatePatternParam,
    ) -> Result<ValidationResult, ServiceError> {
        // Parse language
        let lang = SupportLang::from_str(&param.language).map_err(|_| {
            ServiceError::Internal(format!("Unsupported language: {}", param.language))
        })?;

        // Try to parse the pattern - ast-grep Pattern::new doesn't return Result, it just creates pattern
        let pattern = Pattern::new(&param.pattern, lang);

        // Test pattern if code provided
        let match_result = if let Some(test_code) = &param.test_code {
            self.test_pattern(&pattern, test_code, lang).await.ok()
        } else {
            None
        };

        // For now, we'll assume patterns are always syntactically valid since Pattern::new doesn't fail
        // In the future, we could add more sophisticated validation
        let is_valid = match &match_result {
            Some(_) => true,                   // Pattern matched successfully
            None => param.test_code.is_none(), // No test code provided, so we assume it's valid
        };

        Ok(ValidationResult {
            is_valid,
            match_result,
            analysis: self.analyze_pattern(&param.pattern),
            learning_insights: self.generate_insights(&param.pattern, is_valid),
            suggested_experiments: self.suggest_experiments(&param.pattern),
        })
    }

    async fn test_pattern(
        &self,
        pattern: &Pattern,
        code: &str,
        lang: SupportLang,
    ) -> Result<MatchResult, ServiceError> {
        let ast = AstGrep::new(code, lang);

        if let Some(node_match) = ast.root().find(pattern) {
            Ok(MatchResult::from_node_match(&node_match))
        } else {
            Err(ServiceError::Internal("No matches found".to_string()))
        }
    }

    fn analyze_pattern(&self, pattern: &str) -> PatternAnalysis {
        let complexity = self.calculate_complexity(pattern);
        let metavars = self.analyze_metavariables(pattern);
        let compatibility = self.check_language_compatibility(pattern);

        PatternAnalysis {
            complexity_score: complexity,
            language_compatibility: compatibility,
            metavar_usage: metavars,
            potential_issues: self.identify_issues(pattern),
        }
    }

    fn calculate_complexity(&self, pattern: &str) -> f32 {
        let mut score = 0.0;

        // Base complexity from length
        score += pattern.len() as f32 * 0.01;

        // Metavariable complexity
        score += pattern.matches('$').count() as f32 * 0.1;
        score += pattern.matches("$$$").count() as f32 * 0.2;

        // Structure complexity
        score += pattern.matches('{').count() as f32 * 0.15;
        score += pattern.matches('(').count() as f32 * 0.1;

        // Normalize to 0.0-1.0
        (score / 5.0).min(1.0)
    }

    fn analyze_metavariables(&self, pattern: &str) -> Vec<MetavarInfo> {
        let mut metavars = Vec::new();

        // Find $VAR patterns
        if let Ok(re) = regex::Regex::new(r"\$([A-Z_][A-Z0-9_]*)") {
            for cap in re.captures_iter(pattern) {
                if let Some(name) = cap.get(1) {
                    metavars.push(MetavarInfo {
                        name: name.as_str().to_string(),
                        capture_type: "single_node".to_string(),
                        usage_notes: "Captures a single AST node".to_string(),
                    });
                }
            }
        }

        // Find $$$VAR patterns
        if let Ok(re) = regex::Regex::new(r"\$\$\$([A-Z_][A-Z0-9_]*)") {
            for cap in re.captures_iter(pattern) {
                if let Some(name) = cap.get(1) {
                    metavars.push(MetavarInfo {
                        name: name.as_str().to_string(),
                        capture_type: "multiple_nodes".to_string(),
                        usage_notes: "Captures multiple AST nodes (list)".to_string(),
                    });
                }
            }
        }

        metavars
    }

    fn check_language_compatibility(&self, pattern: &str) -> Vec<String> {
        let mut langs = Vec::new();

        if pattern.contains("function") || pattern.contains("const") || pattern.contains("let") {
            langs.extend(vec!["javascript".to_string(), "typescript".to_string()]);
        }
        if pattern.contains("fn ") || pattern.contains("impl ") {
            langs.push("rust".to_string());
        }
        if pattern.contains("def ") || pattern.contains("class ") {
            langs.push("python".to_string());
        }

        if langs.is_empty() {
            langs = vec![
                "javascript".to_string(),
                "typescript".to_string(),
                "python".to_string(),
                "rust".to_string(),
            ];
        }

        langs
    }

    fn identify_issues(&self, pattern: &str) -> Vec<String> {
        let mut issues = Vec::new();

        if pattern.len() > 100 {
            issues.push("Pattern is quite complex - consider breaking it down".to_string());
        }

        if pattern.matches('$').count() > 5 {
            issues.push("Many metavariables - ensure they're all necessary".to_string());
        }

        if !pattern.contains('$') {
            issues.push("No metavariables - this is exact matching only".to_string());
        }

        issues
    }

    fn generate_insights(&self, pattern: &str, is_valid: bool) -> Vec<LearningInsight> {
        let mut insights = Vec::new();

        if is_valid {
            insights.push(LearningInsight {
                category: "success".to_string(),
                insight: "Pattern syntax is valid!".to_string(),
                actionable_tip:
                    "Try testing this pattern on different code samples to see how it behaves"
                        .to_string(),
            });
        } else {
            insights.push(LearningInsight {
                category: "validation".to_string(),
                insight: "Pattern didn't match the test code".to_string(),
                actionable_tip: "Check if the pattern syntax matches the structure of your test code, or try with different test code".to_string(),
            });
        }

        if pattern.contains("$$$") {
            insights.push(LearningInsight {
                category: "metavariables".to_string(),
                insight: "Using $$$ captures multiple nodes in a list".to_string(),
                actionable_tip: "This is useful for capturing function parameters, array elements, or statement blocks".to_string(),
            });
        }

        // Always provide some general insights
        if pattern.contains('$') {
            insights.push(LearningInsight {
                category: "metavariables".to_string(),
                insight: "Pattern uses metavariables for capturing code elements".to_string(),
                actionable_tip: "Metavariables like $VAR allow you to capture and reuse parts of the matched code".to_string(),
            });
        }

        insights
    }

    fn suggest_experiments(&self, pattern: &str) -> Vec<String> {
        let mut experiments = Vec::new();

        experiments.push("Try this pattern on some sample code".to_string());

        if pattern.contains('$') {
            experiments.push("Try changing metavariable names to see how they capture".to_string());
        }

        if !pattern.contains("$$$") {
            experiments.push("Try adding $$$ to capture multiple items".to_string());
        }

        experiments
    }

    /// Generate educational prompt with LLM hints
    pub fn generate_educational_prompt(
        &self,
        param: ValidatePatternParam,
        result: &ValidationResult,
    ) -> Result<super::prompt_generation::GeneratedPrompt, ServiceError> {
        let config = PromptConfig {
            llm_type: LlmType::Generic,
            interaction_style: InteractionStyle::Educational,
            learning_level: if result.analysis.complexity_score < 0.4 {
                LearningLevel::Beginner
            } else if result.analysis.complexity_score < 0.7 {
                LearningLevel::Intermediate
            } else {
                LearningLevel::Advanced
            },
            focus_areas: vec![
                "pattern_matching".to_string(),
                "ast_understanding".to_string(),
            ],
        };

        let prompt_param = GeneratePromptParam {
            validation_result: result.clone(),
            original_pattern: param.pattern,
            user_goal: param.context,
            config,
            additional_context: None,
        };

        self.prompt_generator.generate_prompt(prompt_param)
    }

    /// Generate quick LLM hint for pattern issues
    pub fn generate_quick_hint(&self, result: &ValidationResult, pattern: &str) -> String {
        self.prompt_generator.generate_quick_hint(result, pattern)
    }
}

impl Default for ValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}
