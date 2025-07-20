//! LLM prompt generation for enhanced learning assistance

use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for different types of LLM interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    pub llm_type: LlmType,
    pub interaction_style: InteractionStyle,
    pub learning_level: LearningLevel,
    pub focus_areas: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmType {
    Claude,
    ChatGpt,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InteractionStyle {
    Educational,
    Debugging,
    Optimization,
    Exploration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LearningLevel {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

/// Generated prompt with context and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedPrompt {
    pub prompt_text: String,
    pub context: PromptContext,
    pub suggested_followups: Vec<String>,
    pub educational_focus: Vec<String>,
    pub technical_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptContext {
    pub pattern: String,
    pub language: String,
    pub complexity_level: String,
    pub success_state: bool,
    pub error_type: Option<String>,
    pub user_goal: String,
}

/// Parameters for prompt generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratePromptParam {
    pub validation_result: ValidationResult,
    pub original_pattern: String,
    pub user_goal: Option<String>,
    pub config: PromptConfig,
    pub additional_context: Option<String>,
}

/// LLM prompt generation engine
#[derive(Clone)]
pub struct PromptGenerator {
    templates: HashMap<String, String>,
}

impl PromptGenerator {
    pub fn new() -> Self {
        let mut templates = HashMap::new();

        // Educational prompt templates
        templates.insert(
            "educational_success".to_string(),
            "Your AST pattern `{pattern}` successfully matched! Here's what this means:\n\n\
             🎯 **Pattern Analysis:**\n\
             - Language: {language}\n\
             - Complexity: {complexity} ({complexity_score:.2}/1.0)\n\
             - Metavariables: {metavar_count}\n\n\
             📚 **Learning Insights:**\n\
             {learning_insights}\n\n\
             🔬 **Try These Experiments:**\n\
             {experiments}\n\n\
             💡 **Next Steps:** {next_steps}"
                .to_string(),
        );

        templates.insert(
            "educational_failure".to_string(),
            "Your AST pattern `{pattern}` didn't match as expected. Let's troubleshoot:\n\n\
             🔍 **What Happened:**\n\
             {failure_analysis}\n\n\
             🛠️ **Potential Fixes:**\n\
             {suggested_fixes}\n\n\
             📖 **Learning Opportunity:**\n\
             {learning_insights}\n\n\
             🎯 **Modified Pattern Suggestions:**\n\
             {pattern_suggestions}"
                .to_string(),
        );

        templates.insert(
            "debugging_context".to_string(),
            "Pattern debugging context for `{pattern}`:\n\n\
             **AST Structure Expectation:**\n\
             {ast_expectation}\n\n\
             **Metavariable Bindings:**\n\
             {metavar_bindings}\n\n\
             **Common Issues:**\n\
             {common_issues}\n\n\
             **Debugging Steps:**\n\
             {debug_steps}"
                .to_string(),
        );

        templates.insert(
            "optimization_context".to_string(),
            "Pattern optimization analysis for `{pattern}`:\n\n\
             **Performance Characteristics:**\n\
             {performance_notes}\n\n\
             **Optimization Opportunities:**\n\
             {optimizations}\n\n\
             **Alternative Patterns:**\n\
             {alternatives}"
                .to_string(),
        );

        Self { templates }
    }

    /// Generate an LLM prompt based on validation results
    pub fn generate_prompt(
        &self,
        param: GeneratePromptParam,
    ) -> Result<GeneratedPrompt, crate::errors::ServiceError> {
        let template_key = self.select_template(&param);
        let template = self.templates.get(&template_key).ok_or_else(|| {
            crate::errors::ServiceError::Internal(format!("Template not found: {template_key}"))
        })?;

        let context = PromptContext {
            pattern: param.original_pattern.clone(),
            language: self.extract_language(&param.validation_result),
            complexity_level: self
                .determine_complexity_level(param.validation_result.analysis.complexity_score),
            success_state: param.validation_result.is_valid,
            error_type: if !param.validation_result.is_valid {
                Some("pattern_mismatch".to_string())
            } else {
                None
            },
            user_goal: param
                .user_goal
                .clone()
                .unwrap_or_else(|| "Learn AST pattern matching".to_string()),
        };

        let prompt_text = self.populate_template(template, &param, &context)?;
        let educational_focus = self.generate_educational_focus(&param);
        let technical_hints = self.generate_technical_hints(&param);
        let suggested_followups = self.generate_followups(&param);

        Ok(GeneratedPrompt {
            prompt_text,
            context,
            suggested_followups,
            educational_focus,
            technical_hints,
        })
    }

    /// Generate a quick hint for pattern errors
    pub fn generate_quick_hint(
        &self,
        validation_result: &ValidationResult,
        pattern: &str,
    ) -> String {
        if validation_result.is_valid {
            format!(
                "✅ Pattern `{}` is working! Complexity: {:.1}/1.0",
                pattern, validation_result.analysis.complexity_score
            )
        } else {
            let issues = &validation_result.analysis.potential_issues;
            let main_issue = issues
                .first()
                .map(String::as_str)
                .unwrap_or("Pattern didn't match test code");

            format!(
                "⚠️  Pattern issue: {} | Hint: {}",
                main_issue,
                validation_result
                    .learning_insights
                    .first()
                    .map(|i| &i.actionable_tip)
                    .unwrap_or(&"Try adjusting the pattern structure".to_string())
            )
        }
    }

    /// Generate debugging context for LLM assistance
    pub fn generate_debugging_context(&self, param: &GeneratePromptParam) -> String {
        let metavar_info = param
            .validation_result
            .analysis
            .metavar_usage
            .iter()
            .map(|mv| format!("${}: {}", mv.name, mv.usage_notes))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "🔧 **AST Pattern Debug Context**\n\n\
             Pattern: `{}`\n\
             Language: {}\n\
             Complexity: {:.2}\n\
             Valid: {}\n\n\
             **Metavariables:**\n{}\n\n\
             **Potential Issues:**\n{}\n\n\
             **Pattern Structure Analysis:**\n\
             {}",
            param.original_pattern,
            self.extract_language(&param.validation_result),
            param.validation_result.analysis.complexity_score,
            param.validation_result.is_valid,
            if metavar_info.is_empty() {
                "None"
            } else {
                &metavar_info
            },
            param.validation_result.analysis.potential_issues.join("\n"),
            self.analyze_pattern_structure(&param.original_pattern)
        )
    }

    fn select_template(&self, param: &GeneratePromptParam) -> String {
        match (
            &param.config.interaction_style,
            param.validation_result.is_valid,
        ) {
            (InteractionStyle::Educational, true) => "educational_success",
            (InteractionStyle::Educational, false) => "educational_failure",
            (InteractionStyle::Debugging, _) => "debugging_context",
            (InteractionStyle::Optimization, _) => "optimization_context",
            (InteractionStyle::Exploration, _) => "educational_success", // Default to educational
        }
        .to_string()
    }

    fn populate_template(
        &self,
        template: &str,
        param: &GeneratePromptParam,
        context: &PromptContext,
    ) -> Result<String, crate::errors::ServiceError> {
        let mut result = template.to_string();

        // Basic substitutions
        result = result.replace("{pattern}", &context.pattern);
        result = result.replace("{language}", &context.language);
        result = result.replace("{complexity}", &context.complexity_level);
        result = result.replace(
            "{complexity_score}",
            &param
                .validation_result
                .analysis
                .complexity_score
                .to_string(),
        );
        result = result.replace(
            "{metavar_count}",
            &param
                .validation_result
                .analysis
                .metavar_usage
                .len()
                .to_string(),
        );

        // Learning insights
        let insights_text = param
            .validation_result
            .learning_insights
            .iter()
            .map(|insight| format!("• {}: {}", insight.insight, insight.actionable_tip))
            .collect::<Vec<_>>()
            .join("\n");
        result = result.replace("{learning_insights}", &insights_text);

        // Experiments
        let experiments_text = param
            .validation_result
            .suggested_experiments
            .iter()
            .enumerate()
            .map(|(i, exp)| format!("{}. {}", i + 1, exp))
            .collect::<Vec<_>>()
            .join("\n");
        result = result.replace("{experiments}", &experiments_text);

        // Context-specific replacements
        if !param.validation_result.is_valid {
            let failure_analysis = self.generate_failure_analysis(param);
            result = result.replace("{failure_analysis}", &failure_analysis);

            let suggested_fixes = self.generate_pattern_fixes(&param.original_pattern);
            result = result.replace("{suggested_fixes}", &suggested_fixes);

            let pattern_suggestions = self.generate_pattern_alternatives(&param.original_pattern);
            result = result.replace("{pattern_suggestions}", &pattern_suggestions);
        }

        // Next steps
        let next_steps = self.generate_next_steps(param);
        result = result.replace("{next_steps}", &next_steps);

        Ok(result)
    }

    fn extract_language(&self, validation_result: &ValidationResult) -> String {
        validation_result
            .analysis
            .language_compatibility
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn determine_complexity_level(&self, score: f32) -> String {
        match score {
            s if s < 0.3 => "Simple",
            s if s < 0.6 => "Moderate",
            s if s < 0.8 => "Complex",
            _ => "Advanced",
        }
        .to_string()
    }

    fn generate_educational_focus(&self, param: &GeneratePromptParam) -> Vec<String> {
        let mut focus = Vec::new();

        if !param.validation_result.analysis.metavar_usage.is_empty() {
            focus.push("Metavariable usage and capture patterns".to_string());
        }

        if param.validation_result.analysis.complexity_score > 0.6 {
            focus.push("Complex pattern structure and AST understanding".to_string());
        }

        if !param.validation_result.is_valid {
            focus.push("Pattern debugging and troubleshooting".to_string());
        }

        focus.push("AST pattern matching fundamentals".to_string());
        focus
    }

    fn generate_technical_hints(&self, param: &GeneratePromptParam) -> Vec<String> {
        let mut hints = Vec::new();

        hints.push(format!(
            "Pattern complexity: {:.2}/1.0",
            param.validation_result.analysis.complexity_score
        ));

        if let Some(first_metavar) = param.validation_result.analysis.metavar_usage.first() {
            hints.push(format!(
                "Primary capture: ${} ({})",
                first_metavar.name, first_metavar.capture_type
            ));
        }

        for issue in &param.validation_result.analysis.potential_issues {
            hints.push(format!("Consideration: {issue}"));
        }

        hints
    }

    fn generate_followups(&self, param: &GeneratePromptParam) -> Vec<String> {
        let mut followups = Vec::new();

        if param.validation_result.is_valid {
            followups.push("Try this pattern on different code samples".to_string());
            followups.push("Experiment with different metavariable names".to_string());
            followups.push("Explore related patterns in the catalog".to_string());
        } else {
            followups.push("Debug why the pattern didn't match".to_string());
            followups.push("Try simplifying the pattern first".to_string());
            followups.push("Check the AST structure of your target code".to_string());
        }

        followups.push("Test pattern variations with edge cases".to_string());
        followups
    }

    fn generate_failure_analysis(&self, param: &GeneratePromptParam) -> String {
        if param.validation_result.match_result.is_none() {
            "The pattern didn't find any matches in the test code. This could mean:\n\
             • The pattern syntax doesn't match the code structure\n\
             • The metavariables aren't capturing the right elements\n\
             • The code uses different syntax than expected"
                .to_string()
        } else {
            "Pattern validation failed for unknown reasons".to_string()
        }
    }

    fn generate_pattern_fixes(&self, pattern: &str) -> String {
        let mut fixes = Vec::new();

        if !pattern.contains("$") {
            fixes.push("• Add metavariables (e.g., $VAR) to make the pattern more flexible");
        }

        if pattern.len() > 50 {
            fixes.push("• Consider breaking down the pattern into smaller, simpler parts");
        }

        if pattern.contains("$$$") && pattern.matches("$$$").count() > 1 {
            fixes.push("• Multiple $$$ captures can be complex - try using one at a time");
        }

        if fixes.is_empty() {
            fixes.push("• Try testing with simpler code examples first");
            fixes.push("• Check that the pattern syntax matches your target language");
        }

        fixes.join("\n")
    }

    fn generate_pattern_alternatives(&self, pattern: &str) -> String {
        let mut alternatives = Vec::new();

        // Suggest simpler version
        if pattern.contains("$") {
            let simple = pattern
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            if simple != pattern {
                alternatives.push(format!("Simpler: `{simple}`"));
            }
        }

        // Suggest with different metavariables
        if pattern.contains("$VAR") {
            alternatives.push(format!("Try: `{}`", pattern.replace("$VAR", "$ITEM")));
        }

        if alternatives.is_empty() {
            alternatives
                .push("Try starting with exact text matching, then add metavariables".to_string());
        }

        alternatives.join("\n")
    }

    fn generate_next_steps(&self, param: &GeneratePromptParam) -> String {
        if param.validation_result.is_valid {
            "Consider exploring more complex patterns or testing with different code samples"
        } else {
            "Focus on understanding why the pattern didn't match, then try simpler variations"
        }
        .to_string()
    }

    fn analyze_pattern_structure(&self, pattern: &str) -> String {
        let mut analysis = Vec::new();

        let metavar_count = pattern.matches("$").count();
        analysis.push(format!("Metavariables: {metavar_count}"));

        let brace_count = pattern.matches("{").count();
        if brace_count > 0 {
            analysis.push(format!("Block structures: {brace_count}"));
        }

        let paren_count = pattern.matches("(").count();
        if paren_count > 0 {
            analysis.push(format!("Function calls/grouping: {paren_count}"));
        }

        analysis.join(" | ")
    }
}

impl Default for PromptGenerator {
    fn default() -> Self {
        Self::new()
    }
}
