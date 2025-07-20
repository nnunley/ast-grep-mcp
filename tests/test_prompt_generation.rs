//! Tests for LLM prompt generation functionality

use ast_grep_mcp::learning::prompt_generation::{InteractionStyle, LearningLevel, LlmType};
use ast_grep_mcp::learning::{
    GeneratePromptParam, LearningInsight, MetavarInfo, PatternAnalysis, PromptConfig,
    PromptGenerator, ValidationResult,
};

#[test]
fn test_prompt_generator_creation() {
    let generator = PromptGenerator::new();

    // Should create without error
    let validation_result = create_sample_validation_result(true);
    let param = create_sample_prompt_param(validation_result);
    let result = generator.generate_prompt(param);

    assert!(result.is_ok());
}

#[test]
fn test_quick_hint_success() {
    let generator = PromptGenerator::new();
    let validation_result = create_sample_validation_result(true);

    let hint = generator.generate_quick_hint(&validation_result, "console.log($VAR)");

    assert!(hint.contains("✅"));
    assert!(hint.contains("Pattern"));
    assert!(hint.contains("Complexity"));
}

#[test]
fn test_quick_hint_failure() {
    let generator = PromptGenerator::new();
    let validation_result = create_sample_validation_result(false);

    let hint = generator.generate_quick_hint(&validation_result, "console.log($VAR)");

    assert!(hint.contains("⚠️"));
    assert!(hint.contains("Pattern issue"));
    assert!(hint.contains("Hint"));
}

#[test]
fn test_educational_prompt_success() {
    let generator = PromptGenerator::new();
    let validation_result = create_sample_validation_result(true);
    let param = create_sample_prompt_param(validation_result);

    let result = generator.generate_prompt(param).unwrap();

    assert!(result.prompt_text.contains("successfully matched"));
    assert!(result.prompt_text.contains("Pattern Analysis"));
    assert!(result.prompt_text.contains("Learning Insights"));
    assert!(!result.educational_focus.is_empty());
    assert!(!result.suggested_followups.is_empty());
}

#[test]
fn test_educational_prompt_failure() {
    let generator = PromptGenerator::new();
    let validation_result = create_sample_validation_result(false);
    let param = create_sample_prompt_param(validation_result);

    let result = generator.generate_prompt(param).unwrap();

    assert!(result.prompt_text.contains("didn't match"));
    assert!(result.prompt_text.contains("troubleshoot"));
    assert!(result.prompt_text.contains("Potential Fixes"));
    assert!(!result.technical_hints.is_empty());
}

#[test]
fn test_debugging_context_generation() {
    let generator = PromptGenerator::new();
    let validation_result = create_sample_validation_result(false);
    let param = create_sample_prompt_param(validation_result);

    let context = generator.generate_debugging_context(&param);

    assert!(context.contains("Debug Context"));
    assert!(context.contains("Pattern:"));
    assert!(context.contains("Language:"));
    assert!(context.contains("Complexity:"));
    assert!(context.contains("Metavariables:"));
}

#[test]
fn test_different_learning_levels() {
    let generator = PromptGenerator::new();

    // Test beginner level (low complexity)
    let mut validation_result = create_sample_validation_result(true);
    validation_result.analysis.complexity_score = 0.2;
    let param = create_sample_prompt_param_with_level(validation_result, LearningLevel::Beginner);
    let result = generator.generate_prompt(param).unwrap();
    assert_eq!(result.context.complexity_level, "Simple");

    // Test advanced level (high complexity)
    let mut validation_result = create_sample_validation_result(true);
    validation_result.analysis.complexity_score = 0.9;
    let param = create_sample_prompt_param_with_level(validation_result, LearningLevel::Advanced);
    let result = generator.generate_prompt(param).unwrap();
    assert_eq!(result.context.complexity_level, "Advanced");
}

#[test]
fn test_different_interaction_styles() {
    let generator = PromptGenerator::new();
    let validation_result = create_sample_validation_result(true);

    // Test debugging style
    let mut param = create_sample_prompt_param(validation_result.clone());
    param.config.interaction_style = InteractionStyle::Debugging;
    let result = generator.generate_prompt(param).unwrap();
    assert!(result.prompt_text.contains("debugging") || result.prompt_text.contains("Debug"));

    // Test optimization style
    let mut param = create_sample_prompt_param(validation_result);
    param.config.interaction_style = InteractionStyle::Optimization;
    let result = generator.generate_prompt(param).unwrap();
    assert!(
        result.prompt_text.contains("optimization") || result.prompt_text.contains("Performance")
    );
}

#[test]
fn test_metavariable_analysis_in_prompts() {
    let generator = PromptGenerator::new();
    let mut validation_result = create_sample_validation_result(true);

    // Add metavariable info
    validation_result.analysis.metavar_usage = vec![
        MetavarInfo {
            name: "VAR".to_string(),
            capture_type: "single_node".to_string(),
            usage_notes: "Captures a single AST node".to_string(),
        },
        MetavarInfo {
            name: "ARGS".to_string(),
            capture_type: "multiple_nodes".to_string(),
            usage_notes: "Captures multiple AST nodes (list)".to_string(),
        },
    ];

    let param = create_sample_prompt_param(validation_result);
    let result = generator.generate_prompt(param).unwrap();

    assert!(result.prompt_text.contains("2")); // metavar count
    assert!(
        result
            .technical_hints
            .iter()
            .any(|hint| hint.contains("VAR"))
    );
}

// Helper functions

fn create_sample_validation_result(is_valid: bool) -> ValidationResult {
    ValidationResult {
        is_valid,
        match_result: if is_valid {
            Some(create_sample_match_result())
        } else {
            None
        },
        analysis: PatternAnalysis {
            complexity_score: 0.5,
            language_compatibility: vec!["javascript".to_string()],
            metavar_usage: vec![MetavarInfo {
                name: "VAR".to_string(),
                capture_type: "single_node".to_string(),
                usage_notes: "Captures a single AST node".to_string(),
            }],
            potential_issues: if is_valid {
                vec![]
            } else {
                vec!["Pattern didn't match test code".to_string()]
            },
        },
        learning_insights: vec![LearningInsight {
            category: if is_valid { "success" } else { "validation" }.to_string(),
            insight: if is_valid {
                "Pattern syntax is valid!"
            } else {
                "Pattern didn't match the test code"
            }
            .to_string(),
            actionable_tip: "Try testing this pattern on different code samples".to_string(),
        }],
        suggested_experiments: vec![
            "Try this pattern on some sample code".to_string(),
            "Try changing metavariable names to see how they capture".to_string(),
        ],
    }
}

fn create_sample_match_result() -> ast_grep_mcp::types::MatchResult {
    ast_grep_mcp::types::MatchResult {
        text: "console.log('hello')".to_string(),
        start_line: 1,
        start_col: 0,
        end_line: 1,
        end_col: 20,
        vars: std::collections::HashMap::new(),
        context_before: None,
        context_after: None,
    }
}

fn create_sample_prompt_param(validation_result: ValidationResult) -> GeneratePromptParam {
    GeneratePromptParam {
        validation_result,
        original_pattern: "console.log($VAR)".to_string(),
        user_goal: Some("Learn AST pattern matching".to_string()),
        config: PromptConfig {
            llm_type: LlmType::Generic,
            interaction_style: InteractionStyle::Educational,
            learning_level: LearningLevel::Intermediate,
            focus_areas: vec!["pattern_matching".to_string()],
        },
        additional_context: None,
    }
}

fn create_sample_prompt_param_with_level(
    validation_result: ValidationResult,
    level: LearningLevel,
) -> GeneratePromptParam {
    let mut param = create_sample_prompt_param(validation_result);
    param.config.learning_level = level;
    param
}
