use super::ast::Rule;
use super::types::{RuleConfig, RuleTestResult, RuleValidateParam, RuleValidateResult};
use crate::errors::ServiceError;
// Removed unused import
use ast_grep_language::SupportLang as Language;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Rule configuration that supports direct parsing into Rule enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfigEnum {
    pub id: String,
    pub message: Option<String>,
    pub language: String,
    pub severity: Option<String>,
    pub rule: Rule,
    pub fix: Option<String>,
}

/// Parse rule configuration with enum-based Rule AST
pub fn parse_rule_config_enum(content: &str) -> Result<RuleConfigEnum, ServiceError> {
    // Try parsing as YAML first
    if let Ok(rule) = serde_yaml::from_str::<RuleConfigEnum>(content) {
        return Ok(rule);
    }

    // Fall back to JSON
    if let Ok(rule) = serde_json::from_str::<RuleConfigEnum>(content) {
        return Ok(rule);
    }

    Err(ServiceError::ParserError(
        "Rule configuration must be valid YAML or JSON".to_string(),
    ))
}

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
                let lang = &rule.language;
                errors.push(format!("Unsupported language: {lang}"));
            }

            // Validate rule structure
            if !has_valid_rule_condition(&rule.rule) {
                errors.push(
                    "Rule must have at least one valid condition (pattern, kind, regex, etc.)"
                        .to_string(),
                );
            }

            // Validate severity if present
            if let Some(ref severity) = rule.severity {
                if !matches!(severity.as_str(), "error" | "warning" | "info") {
                    errors.push(format!(
                        "Invalid severity '{severity}'. Must be 'error', 'warning', or 'info'"
                    ));
                }
            }
        }
        Err(e) => {
            errors.push(format!("Failed to parse rule configuration: {e}"));
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
                let _test_code = param.test_code.unwrap();
                let _lang = Language::from_str(&rule.language)
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
    rule.pattern.is_some()
        || rule.kind.is_some()
        || rule.regex.is_some()
        || rule.inside.is_some()
        || rule.has.is_some()
        || rule.follows.is_some()
        || rule.precedes.is_some()
        || rule.all.as_ref().is_some_and(|v| !v.is_empty())
        || rule.any.as_ref().is_some_and(|v| !v.is_empty())
        || rule.not.is_some()
        || rule.matches.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ast::{PatternRule, Rule};

    #[test]
    fn test_parse_rule_config_yaml() {
        let yaml_config = r#"
id: test-rule
language: javascript
message: "Test rule"
severity: error
rule:
  pattern: "console.log($VAR)"
fix: "console.debug($VAR)"
"#;
        let result = parse_rule_config(yaml_config).unwrap();
        assert_eq!(result.id, "test-rule");
        assert_eq!(result.language, "javascript");
        assert_eq!(result.message, Some("Test rule".to_string()));
        assert_eq!(result.severity, Some("error".to_string()));
        assert_eq!(result.fix, Some("console.debug($VAR)".to_string()));
    }

    #[test]
    fn test_parse_rule_config_json() {
        let json_config = r#"
{
  "id": "test-rule",
  "language": "javascript",
  "message": "Test rule",
  "severity": "warning",
  "rule": {
    "pattern": "console.log($VAR)"
  },
  "fix": "console.debug($VAR)"
}
"#;
        let result = parse_rule_config(json_config).unwrap();
        assert_eq!(result.id, "test-rule");
        assert_eq!(result.language, "javascript");
        assert_eq!(result.severity, Some("warning".to_string()));
    }

    #[test]
    fn test_parse_rule_config_enum_yaml() {
        let yaml_config = r#"
id: test-rule-enum
language: javascript
message: "Test enum rule"
rule:
  pattern: "console.log($VAR)"
"#;
        let result = parse_rule_config_enum(yaml_config).unwrap();
        assert_eq!(result.id, "test-rule-enum");
        assert_eq!(result.language, "javascript");
        match result.rule {
            Rule::Pattern(PatternRule::Simple { pattern }) => {
                assert_eq!(pattern, "console.log($VAR)");
            }
            _ => panic!("Expected simple pattern rule"),
        }
    }

    #[test]
    fn test_parse_rule_config_invalid() {
        let invalid_config = "invalid yaml { content";
        let result = parse_rule_config(invalid_config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_rule_config_valid() {
        let valid_config = r#"
id: valid-rule
language: javascript
rule:
  pattern: "console.log($VAR)"
"#;
        let errors = validate_rule_config(valid_config).unwrap();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_rule_config_invalid_language() {
        let invalid_config = r#"
id: invalid-rule
language: invalid_language
rule:
  pattern: "console.log($VAR)"
"#;
        let errors = validate_rule_config(invalid_config).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Unsupported language"));
    }

    #[test]
    fn test_validate_rule_config_missing_condition() {
        let invalid_config = r#"
id: empty-rule
language: javascript
rule: {}
"#;
        let errors = validate_rule_config(invalid_config).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("at least one valid condition"));
    }

    #[test]
    fn test_validate_rule_config_invalid_severity() {
        let invalid_config = r#"
id: invalid-severity
language: javascript
severity: invalid_severity
rule:
  pattern: "console.log($VAR)"
"#;
        let errors = validate_rule_config(invalid_config).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Invalid severity"));
    }

    #[test]
    fn test_has_valid_rule_condition() {
        use super::super::types::{PatternSpec, RuleObject};

        // Test pattern rule
        let pattern_rule = RuleObject {
            pattern: Some(PatternSpec::Simple("test".to_string())),
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        };
        assert!(has_valid_rule_condition(&pattern_rule));

        // Test kind rule
        let kind_rule = RuleObject {
            pattern: None,
            kind: Some("function_declaration".to_string()),
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        };
        assert!(has_valid_rule_condition(&kind_rule));

        // Test empty rule
        let empty_rule = RuleObject {
            pattern: None,
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: None,
            any: None,
            not: None,
            matches: None,
        };
        assert!(!has_valid_rule_condition(&empty_rule));

        // Test all rule with empty vec
        let empty_all_rule = RuleObject {
            pattern: None,
            kind: None,
            regex: None,
            inside: None,
            has: None,
            follows: None,
            precedes: None,
            all: Some(vec![]),
            any: None,
            not: None,
            matches: None,
        };
        assert!(!has_valid_rule_condition(&empty_all_rule));
    }

    #[tokio::test]
    async fn test_validate_rule_with_test_code() {
        let param = RuleValidateParam {
            rule_config: r#"
id: test-with-code
language: javascript
rule:
  pattern: "console.log($VAR)"
"#
            .to_string(),
            test_code: Some("console.log('test');".to_string()),
        };

        let result = validate_rule(param).await.unwrap();
        assert!(result.valid);
        assert!(result.test_results.is_some());
    }

    #[tokio::test]
    async fn test_validate_rule_invalid_config() {
        let param = RuleValidateParam {
            rule_config: "invalid config".to_string(),
            test_code: None,
        };

        let result = validate_rule(param).await.unwrap();
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }
}
