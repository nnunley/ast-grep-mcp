use crate::errors::ServiceError;
use super::types::{RuleConfig, RuleValidateParam, RuleValidateResult, RuleTestResult};
// Removed unused import
use ast_grep_language::SupportLang as Language;
use std::str::FromStr;

pub fn parse_rule_config(content: &str) -> Result<RuleConfig, ServiceError> {
    // Try parsing as YAML first
    if let Ok(rule) = serde_yaml::from_str::<RuleConfig>(content) {
        return Ok(rule);
    }

    // Fall back to JSON
    if let Ok(rule) = serde_json::from_str::<RuleConfig>(content) {
        return Ok(rule);
    }

    Err(ServiceError::ParserError(
        "Rule configuration must be valid YAML or JSON".to_string(),
    ))
}

pub fn validate_rule_config(content: &str) -> Result<Vec<String>, ServiceError> {
    let mut errors = Vec::new();

    // Try to parse the rule config
    match parse_rule_config(content) {
        Ok(rule) => {
            // Validate language
            if Language::from_str(&rule.language).is_err() {
                errors.push(format!("Unsupported language: {}", rule.language));
            }

            // Validate rule structure
            if !has_valid_rule_condition(&rule.rule) {
                errors.push("Rule must have at least one valid condition (pattern, kind, regex, etc.)".to_string());
            }

            // Validate severity if present
            if let Some(ref severity) = rule.severity {
                if !matches!(severity.as_str(), "error" | "warning" | "info") {
                    errors.push(format!("Invalid severity '{}'. Must be 'error', 'warning', or 'info'", severity));
                }
            }
        }
        Err(e) => {
            errors.push(format!("Failed to parse rule configuration: {}", e));
        }
    }

    Ok(errors)
}

pub async fn validate_rule(param: RuleValidateParam) -> Result<RuleValidateResult, ServiceError> {
    let errors = validate_rule_config(&param.rule_config)?;
    let valid = errors.is_empty();

    let test_results = if valid && param.test_code.is_some() {
        // If rule is valid and test code is provided, test it
        match parse_rule_config(&param.rule_config) {
            Ok(rule) => {
                let test_code = param.test_code.unwrap();
                let lang = Language::from_str(&rule.language)
                    .map_err(|_| ServiceError::ParserError("Invalid language".to_string()))?;
                
                // This would need the evaluation engine to work properly
                // For now, return a mock result
                Some(RuleTestResult {
                    matches_found: 0,
                    sample_matches: vec![],
                })
            }
            Err(_) => None,
        }
    } else {
        None
    };

    Ok(RuleValidateResult {
        valid,
        errors,
        test_results,
    })
}

fn has_valid_rule_condition(rule: &super::types::RuleObject) -> bool {
    rule.pattern.is_some() ||
        rule.kind.is_some() ||
        rule.regex.is_some() ||
        rule.inside.is_some() ||
        rule.has.is_some() ||
        rule.follows.is_some() ||
        rule.precedes.is_some() ||
        rule.all.as_ref().is_some_and(|v| !v.is_empty()) ||
        rule.any.as_ref().is_some_and(|v| !v.is_empty()) ||
        rule.not.is_some() ||
        rule.matches.is_some()
}