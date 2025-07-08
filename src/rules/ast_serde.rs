use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::HashMap;

use super::ast::{PatternRule, Rule};

/// Helper structure for deserializing rules from YAML/JSON
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RuleDeserializer {
    // Simple string patterns (for backward compatibility)
    SimplePattern(String),
    // Object with fields
    Object(HashMap<String, Value>),
}

impl From<RuleDeserializer> for Rule {
    fn from(deserializer: RuleDeserializer) -> Self {
        match deserializer {
            RuleDeserializer::SimplePattern(pattern) => {
                Rule::Pattern(PatternRule::Simple { pattern })
            }
            RuleDeserializer::Object(mut map) => {
                // Check for pattern field
                if let Some(pattern_val) = map.remove("pattern") {
                    match pattern_val {
                        Value::String(pattern) => {
                            return Rule::Pattern(PatternRule::Simple { pattern });
                        }
                        Value::Object(pattern_obj) => {
                            // Advanced pattern with context/selector
                            let pattern = pattern_obj
                                .get("context")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let context = Some(pattern.clone());
                            let selector = pattern_obj
                                .get("selector")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            let strictness = pattern_obj
                                .get("strictness")
                                .and_then(|v| v.as_str())
                                .map(String::from);

                            return Rule::Pattern(PatternRule::Advanced {
                                pattern,
                                context,
                                selector,
                                strictness,
                            });
                        }
                        _ => {}
                    }
                }

                // Check for kind
                if let Some(Value::String(kind)) = map.remove("kind") {
                    return Rule::Kind(kind);
                }

                // Check for regex
                if let Some(Value::String(regex)) = map.remove("regex") {
                    return Rule::Regex(regex);
                }

                // Check for matches
                if let Some(Value::String(matches)) = map.remove("matches") {
                    return Rule::Matches(matches);
                }

                // Check for composite rules
                if let Some(all_val) = map.remove("all") {
                    if let Ok(rules) = serde_json::from_value::<Vec<RuleDeserializer>>(all_val) {
                        return Rule::All(rules.into_iter().map(Rule::from).collect());
                    }
                }

                if let Some(any_val) = map.remove("any") {
                    if let Ok(rules) = serde_json::from_value::<Vec<RuleDeserializer>>(any_val) {
                        return Rule::Any(rules.into_iter().map(Rule::from).collect());
                    }
                }

                if let Some(not_val) = map.remove("not") {
                    if let Ok(rule) = serde_json::from_value::<RuleDeserializer>(not_val) {
                        return Rule::Not(Box::new(Rule::from(rule)));
                    }
                }

                // For relational rules, we need to handle them specially
                // The YAML structure is typically:
                // rule:
                //   pattern: something
                //   inside:
                //     pattern: container

                // Check if this is a rule with relational conditions
                let has_pattern = map.contains_key("pattern");
                let has_inside = map.contains_key("inside");
                let has_has = map.contains_key("has");
                let has_follows = map.contains_key("follows");
                let has_precedes = map.contains_key("precedes");

                if has_pattern && (has_inside || has_has || has_follows || has_precedes) {
                    // This is a relational rule with a base pattern
                    // For now, return a placeholder - proper implementation would parse both parts
                    return Rule::Pattern(PatternRule::Simple {
                        pattern: "RELATIONAL_PLACEHOLDER".to_string(),
                    });
                }

                // Default to empty All rule
                Rule::All(vec![])
            }
        }
    }
}

/// Custom deserializer for Rule that uses RuleDeserializer
impl<'de> Deserialize<'de> for Rule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = RuleDeserializer::deserialize(deserializer)?;
        Ok(Rule::from(helper))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_simple_pattern() {
        let json_value = json!("console.log($VAR)");
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::Pattern(PatternRule::Simple { pattern }) => {
                assert_eq!(pattern, "console.log($VAR)");
            }
            _ => panic!("Expected simple pattern rule"),
        }
    }

    #[test]
    fn test_deserialize_pattern_object() {
        let json_value = json!({
            "pattern": "console.log($VAR)"
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::Pattern(PatternRule::Simple { pattern }) => {
                assert_eq!(pattern, "console.log($VAR)");
            }
            _ => panic!("Expected simple pattern rule"),
        }
    }

    #[test]
    fn test_deserialize_advanced_pattern() {
        let json_value = json!({
            "pattern": {
                "context": "function $FUNC() { $$$ }",
                "selector": "assignment_expression"
            }
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::Pattern(PatternRule::Advanced {
                pattern,
                context,
                selector,
                ..
            }) => {
                assert_eq!(pattern, "function $FUNC() { $$$ }");
                assert_eq!(context, Some("function $FUNC() { $$$ }".to_string()));
                assert_eq!(selector, Some("assignment_expression".to_string()));
            }
            _ => panic!("Expected advanced pattern rule"),
        }
    }

    #[test]
    fn test_deserialize_kind_rule() {
        let json_value = json!({
            "kind": "function_declaration"
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::Kind(kind) => {
                assert_eq!(kind, "function_declaration");
            }
            _ => panic!("Expected kind rule"),
        }
    }

    #[test]
    fn test_deserialize_regex_rule() {
        let json_value = json!({
            "regex": "TODO|FIXME"
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::Regex(regex) => {
                assert_eq!(regex, "TODO|FIXME");
            }
            _ => panic!("Expected regex rule"),
        }
    }

    #[test]
    fn test_deserialize_matches_rule() {
        let json_value = json!({
            "matches": "rule-id"
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::Matches(matches) => {
                assert_eq!(matches, "rule-id");
            }
            _ => panic!("Expected matches rule"),
        }
    }

    #[test]
    fn test_deserialize_all_rule() {
        let json_value = json!({
            "all": [
                {"pattern": "console.log($VAR)"},
                {"kind": "function_declaration"}
            ]
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::All(rules) => {
                assert_eq!(rules.len(), 2);
                match &rules[0] {
                    Rule::Pattern(PatternRule::Simple { pattern }) => {
                        assert_eq!(pattern, "console.log($VAR)");
                    }
                    _ => panic!("Expected pattern rule"),
                }
                match &rules[1] {
                    Rule::Kind(kind) => {
                        assert_eq!(kind, "function_declaration");
                    }
                    _ => panic!("Expected kind rule"),
                }
            }
            _ => panic!("Expected all rule"),
        }
    }

    #[test]
    fn test_deserialize_any_rule() {
        let json_value = json!({
            "any": [
                {"pattern": "console.log($VAR)"},
                {"pattern": "console.error($VAR)"}
            ]
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::Any(rules) => {
                assert_eq!(rules.len(), 2);
            }
            _ => panic!("Expected any rule"),
        }
    }

    #[test]
    fn test_deserialize_not_rule() {
        let json_value = json!({
            "not": {
                "pattern": "console.log($VAR)"
            }
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::Not(boxed_rule) => match boxed_rule.as_ref() {
                Rule::Pattern(PatternRule::Simple { pattern }) => {
                    assert_eq!(pattern, "console.log($VAR)");
                }
                _ => panic!("Expected pattern rule inside not"),
            },
            _ => panic!("Expected not rule"),
        }
    }

    #[test]
    fn test_deserialize_empty_object() {
        let json_value = json!({});
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        match rule {
            Rule::All(rules) => {
                assert!(rules.is_empty());
            }
            _ => panic!("Expected empty all rule for empty object"),
        }
    }

    #[test]
    fn test_relational_placeholder() {
        let json_value = json!({
            "pattern": "console.log($VAR)",
            "inside": {
                "pattern": "function $FUNC() { $$$ }"
            }
        });
        let rule: Rule = serde_json::from_value(json_value).unwrap();

        // This should be handled as a relational placeholder
        match rule {
            Rule::Pattern(PatternRule::Simple { pattern }) => {
                assert_eq!(pattern, "RELATIONAL_PLACEHOLDER");
            }
            _ => panic!("Expected relational placeholder pattern"),
        }
    }

    #[test]
    fn test_yaml_deserialization() {
        let yaml_str = r#"
pattern: "console.log($VAR)"
"#;
        let rule: Rule = serde_yaml::from_str(yaml_str).unwrap();

        match rule {
            Rule::Pattern(PatternRule::Simple { pattern }) => {
                assert_eq!(pattern, "console.log($VAR)");
            }
            _ => panic!("Expected simple pattern rule from YAML"),
        }
    }

    #[test]
    fn test_yaml_complex_rule() {
        let yaml_str = r#"
all:
  - pattern: "console.log($VAR)"
  - kind: "function_declaration"
"#;
        let rule: Rule = serde_yaml::from_str(yaml_str).unwrap();

        match rule {
            Rule::All(rules) => {
                assert_eq!(rules.len(), 2);
            }
            _ => panic!("Expected all rule from YAML"),
        }
    }
}
