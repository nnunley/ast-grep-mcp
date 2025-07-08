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
