use serde::{Deserialize, Serialize};

/// Enum-based AST representation for rules
/// This provides a cleaner, more type-safe way to represent rule structures
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Rule {
    /// Simple pattern matching rule
    Pattern(PatternRule),
    /// Match nodes by their AST kind
    Kind(String),
    /// Match nodes by regex pattern
    Regex(String),
    /// Reference to another rule by ID
    Matches(String),
    /// Composite rule - ALL conditions must match
    All(Vec<Rule>),
    /// Composite rule - ANY condition must match
    Any(Vec<Rule>),
    /// Negation rule - condition must NOT match
    Not(Box<Rule>),
    /// Relational rule - match nodes inside another pattern
    Inside {
        rule: Box<Rule>,
        #[serde(rename = "inside")]
        inside_of: Box<Rule>,
    },
    /// Relational rule - match nodes that contain another pattern
    Has {
        rule: Box<Rule>,
        #[serde(rename = "has")]
        contains: Box<Rule>,
    },
    /// Relational rule - match nodes that follow another pattern
    Follows {
        rule: Box<Rule>,
        #[serde(rename = "follows")]
        after: Box<Rule>,
    },
    /// Relational rule - match nodes that precede another pattern
    Precedes {
        rule: Box<Rule>,
        #[serde(rename = "precedes")]
        before: Box<Rule>,
    },
}

/// Pattern rule with optional advanced features
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatternRule {
    /// Simple pattern string
    Simple { pattern: String },
    /// Advanced pattern with context and selector
    Advanced {
        pattern: String,
        context: Option<String>,
        selector: Option<String>,
        strictness: Option<String>,
    },
}

impl Rule {
    /// Check if this rule has any conditions
    pub fn has_condition(&self) -> bool {
        match self {
            Rule::Pattern(_) | Rule::Kind(_) | Rule::Regex(_) | Rule::Matches(_) => true,
            Rule::All(rules) | Rule::Any(rules) => !rules.is_empty(),
            Rule::Not(_)
            | Rule::Inside { .. }
            | Rule::Has { .. }
            | Rule::Follows { .. }
            | Rule::Precedes { .. } => true,
        }
    }

    /// Extract pattern string if this is a pattern rule
    pub fn extract_pattern(&self) -> Option<String> {
        match self {
            Rule::Pattern(pattern_rule) => match pattern_rule {
                PatternRule::Simple { pattern } => Some(pattern.clone()),
                PatternRule::Advanced { pattern, .. } => Some(pattern.clone()),
            },
            _ => None,
        }
    }

    /// Check if this is a simple pattern rule (no other conditions)
    pub fn is_simple_pattern(&self) -> bool {
        matches!(self, Rule::Pattern(_))
    }

    /// Recursively extract all patterns from composite rules
    pub fn extract_all_patterns(&self) -> Vec<String> {
        let mut patterns = Vec::new();

        match self {
            Rule::Pattern(_) => {
                if let Some(pattern) = self.extract_pattern() {
                    patterns.push(pattern);
                }
            }
            Rule::All(rules) | Rule::Any(rules) => {
                for rule in rules {
                    patterns.extend(rule.extract_all_patterns());
                }
            }
            Rule::Not(rule)
            | Rule::Inside { rule, .. }
            | Rule::Has { rule, .. }
            | Rule::Follows { rule, .. }
            | Rule::Precedes { rule, .. } => {
                patterns.extend(rule.extract_all_patterns());
            }
            Rule::Kind(_) | Rule::Regex(_) | Rule::Matches(_) => {}
        }

        patterns
    }
}

/// Convert from the old RuleObject structure to the new Rule enum
impl From<super::types::RuleObject> for Rule {
    fn from(obj: super::types::RuleObject) -> Self {
        // Pattern rule
        if let Some(pattern_spec) = obj.pattern {
            return match pattern_spec {
                super::types::PatternSpec::Simple(pattern) => {
                    Rule::Pattern(PatternRule::Simple { pattern })
                }
                super::types::PatternSpec::Advanced {
                    context,
                    selector,
                    strictness,
                } => Rule::Pattern(PatternRule::Advanced {
                    pattern: context,
                    context: Some(context),
                    selector,
                    strictness,
                }),
            };
        }

        // Kind rule
        if let Some(kind) = obj.kind {
            return Rule::Kind(kind);
        }

        // Regex rule
        if let Some(regex) = obj.regex {
            return Rule::Regex(regex);
        }

        // Matches rule
        if let Some(matches) = obj.matches {
            return Rule::Matches(matches);
        }

        // Composite rules
        if let Some(all_rules) = obj.all {
            return Rule::All(all_rules.into_iter().map(Rule::from).collect());
        }

        if let Some(any_rules) = obj.any {
            return Rule::Any(any_rules.into_iter().map(Rule::from).collect());
        }

        if let Some(not_rule) = obj.not {
            return Rule::Not(Box::new(Rule::from(*not_rule)));
        }

        // Relational rules - for now, create a placeholder
        // TODO: Properly handle relational rules once we understand the exact structure
        if obj.inside.is_some()
            || obj.has.is_some()
            || obj.follows.is_some()
            || obj.precedes.is_some()
        {
            // Return a simple pattern that will never match as a placeholder
            return Rule::Pattern(PatternRule::Simple {
                pattern: "$$PLACEHOLDER$$".to_string(),
            });
        }

        // Default to an empty All rule if nothing matches
        Rule::All(vec![])
    }
}

/// Convert back to RuleObject for compatibility
impl From<Rule> for super::types::RuleObject {
    fn from(rule: Rule) -> Self {
        let mut obj = super::types::RuleObject {
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

        match rule {
            Rule::Pattern(pattern_rule) => {
                obj.pattern = Some(match pattern_rule {
                    PatternRule::Simple { pattern } => super::types::PatternSpec::Simple(pattern),
                    PatternRule::Advanced {
                        pattern,
                        selector,
                        strictness,
                        ..
                    } => super::types::PatternSpec::Advanced {
                        context: pattern,
                        selector,
                        strictness,
                    },
                });
            }
            Rule::Kind(kind) => obj.kind = Some(kind),
            Rule::Regex(regex) => obj.regex = Some(regex),
            Rule::Matches(matches) => obj.matches = Some(matches),
            Rule::All(rules) => {
                obj.all = Some(
                    rules
                        .into_iter()
                        .map(super::types::RuleObject::from)
                        .collect(),
                )
            }
            Rule::Any(rules) => {
                obj.any = Some(
                    rules
                        .into_iter()
                        .map(super::types::RuleObject::from)
                        .collect(),
                )
            }
            Rule::Not(rule) => obj.not = Some(Box::new(super::types::RuleObject::from(*rule))),
            // TODO: Handle relational rules properly
            Rule::Inside { .. }
            | Rule::Has { .. }
            | Rule::Follows { .. }
            | Rule::Precedes { .. } => {
                // For now, return empty object
            }
        }

        obj
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_pattern_rule() {
        let rule = Rule::Pattern(PatternRule::Simple {
            pattern: "console.log($VAR)".to_string(),
        });
        assert!(rule.has_condition());
        assert!(rule.is_simple_pattern());
        assert_eq!(
            rule.extract_pattern(),
            Some("console.log($VAR)".to_string())
        );
    }

    #[test]
    fn test_composite_all_rule() {
        let rule = Rule::All(vec![
            Rule::Pattern(PatternRule::Simple {
                pattern: "function $NAME()".to_string(),
            }),
            Rule::Kind("function_declaration".to_string()),
        ]);
        assert!(rule.has_condition());
        assert!(!rule.is_simple_pattern());
        let patterns = rule.extract_all_patterns();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0], "function $NAME()");
    }

    #[test]
    fn test_nested_rules() {
        let rule = Rule::Not(Box::new(Rule::Any(vec![
            Rule::Pattern(PatternRule::Simple {
                pattern: "console.log($X)".to_string(),
            }),
            Rule::Pattern(PatternRule::Simple {
                pattern: "console.error($X)".to_string(),
            }),
        ])));
        assert!(rule.has_condition());
        let patterns = rule.extract_all_patterns();
        assert_eq!(patterns.len(), 2);
    }
}
