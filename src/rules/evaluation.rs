use super::types::{PatternSpec, RuleObject};
use crate::errors::ServiceError;
use crate::types::MatchResult;
use ast_grep_core::{AstGrep, Pattern};
use ast_grep_language::SupportLang as Language;
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct RuleEvaluator {
    pattern_cache: Arc<Mutex<HashMap<String, Pattern>>>,
}

impl Default for RuleEvaluator {
    fn default() -> Self {
        Self {
            pattern_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl RuleEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn evaluate_rule_against_code(
        &self,
        rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Handle different rule types
        if let Some(pattern_spec) = &rule.pattern {
            // Simple pattern rule
            self.evaluate_pattern_rule(pattern_spec, code, lang)
        } else if let Some(all_rules) = &rule.all {
            // ALL composite rule - node must match ALL sub-rules
            self.evaluate_all_rule(all_rules, code, lang)
        } else if let Some(any_rules) = &rule.any {
            // ANY composite rule - node must match ANY sub-rule
            self.evaluate_any_rule(any_rules, code, lang)
        } else if let Some(not_rule) = &rule.not {
            // NOT composite rule - find nodes that DON'T match the sub-rule
            self.evaluate_not_rule(not_rule, code, lang)
        } else if let Some(kind) = &rule.kind {
            // Kind rule - match nodes by AST kind (simplified implementation)
            self.evaluate_kind_rule(kind, code, lang)
        } else if let Some(regex) = &rule.regex {
            // Regex rule - match nodes by text content
            self.evaluate_regex_rule(regex, code, lang)
        } else {
            Err(ServiceError::ParserError(
                "Rule must have at least one condition".into(),
            ))
        }
    }

    fn evaluate_pattern_rule(
        &self,
        pattern_spec: &PatternSpec,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let pattern_str = match pattern_spec {
            PatternSpec::Simple(pattern) => pattern.clone(),
            PatternSpec::Advanced { context, .. } => context.clone(),
        };

        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(&pattern_str, lang)?;

        let matches: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| MatchResult::from_node_match(&node))
            .collect();

        Ok(matches)
    }

    fn evaluate_all_rule(
        &self,
        all_rules: &[RuleObject],
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        if all_rules.is_empty() {
            return Ok(vec![]);
        }

        // Start with matches from the first rule
        let mut intersection_matches =
            self.evaluate_rule_against_code(&all_rules[0], code, lang)?;

        // For each subsequent rule, find intersection with current matches
        for rule in &all_rules[1..] {
            let rule_matches = self.evaluate_rule_against_code(rule, code, lang)?;
            intersection_matches = self.intersect_matches(intersection_matches, rule_matches);
        }

        Ok(intersection_matches)
    }

    fn evaluate_any_rule(
        &self,
        any_rules: &[RuleObject],
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let mut all_matches = Vec::new();

        // Collect matches from all rules
        for rule in any_rules {
            let mut rule_matches = self.evaluate_rule_against_code(rule, code, lang)?;
            all_matches.append(&mut rule_matches);
        }

        // Remove duplicates based on text content and position
        all_matches.sort_by_key(|m| (m.start_line, m.start_col, m.text.clone()));
        all_matches.dedup_by_key(|m| (m.start_line, m.start_col, m.text.clone()));

        Ok(all_matches)
    }

    fn evaluate_not_rule(
        &self,
        not_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Get all potential nodes by using a catch-all pattern or kind
        // For simplicity, we'll look for any identifier or statement
        let catch_all_pattern = "$_";
        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(catch_all_pattern, lang)?;

        let all_nodes: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| MatchResult::from_node_match(&node))
            .collect();

        // Get matches from the NOT rule
        let not_matches = self.evaluate_rule_against_code(not_rule, code, lang)?;

        // Return nodes that don't match the NOT rule
        let filtered_matches: Vec<MatchResult> = all_nodes
            .into_iter()
            .filter(|node| {
                !not_matches.iter().any(|not_match| {
                    node.start_line == not_match.start_line
                        && node.start_col == not_match.start_col
                        && node.text == not_match.text
                })
            })
            .collect();

        Ok(filtered_matches)
    }

    fn evaluate_kind_rule(
        &self,
        kind: &str,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Create a pattern that matches any node of the specified kind
        // This is a simplified implementation
        let kind_pattern = format!("{kind}()");
        let ast = AstGrep::new(code, lang);

        // Try to create a pattern for the kind
        match self.get_or_create_pattern(&kind_pattern, lang) {
            Ok(pattern) => {
                let matches: Vec<MatchResult> = ast
                    .root()
                    .find_all(pattern)
                    .map(|node| {
                        let vars: HashMap<String, String> = node.get_env().clone().into();
                        let start_pos = node.get_node().start_pos();
                        let end_pos = node.get_node().end_pos();

                        MatchResult {
                            text: node.text().to_string(),
                            start_line: start_pos.line(),
                            end_line: end_pos.line(),
                            start_col: start_pos.column(&node),
                            end_col: end_pos.column(&node),
                            vars,
                        }
                    })
                    .collect();

                Ok(matches)
            }
            Err(_) => {
                // Fallback: use a more generic approach
                Ok(vec![])
            }
        }
    }

    fn evaluate_regex_rule(
        &self,
        regex_pattern: &str,
        code: &str,
        _lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let regex = Regex::new(regex_pattern)?;
        let mut matches = Vec::new();

        // Find all regex matches in the code
        for (line_idx, line) in code.lines().enumerate() {
            for regex_match in regex.find_iter(line) {
                matches.push(MatchResult {
                    text: regex_match.as_str().to_string(),
                    start_line: line_idx + 1,
                    end_line: line_idx + 1,
                    start_col: regex_match.start(),
                    end_col: regex_match.end(),
                    vars: HashMap::new(),
                });
            }
        }

        Ok(matches)
    }

    fn get_or_create_pattern(
        &self,
        pattern_str: &str,
        lang: Language,
    ) -> Result<Pattern, ServiceError> {
        let cache_key = format!("{lang}:{pattern_str}");

        // Try to get from cache first
        {
            let cache = self.pattern_cache.lock().unwrap();
            if let Some(pattern) = cache.get(&cache_key) {
                return Ok(pattern.clone());
            }
        }

        // Create new pattern
        let pattern = Pattern::new(pattern_str, lang);

        // Store in cache
        {
            let mut cache = self.pattern_cache.lock().unwrap();
            cache.insert(cache_key, pattern.clone());
        }

        Ok(pattern)
    }

    fn intersect_matches(
        &self,
        matches1: Vec<MatchResult>,
        matches2: Vec<MatchResult>,
    ) -> Vec<MatchResult> {
        matches1
            .into_iter()
            .filter(|m1| {
                matches2.iter().any(|m2| {
                    // Check if matches overlap or are the same
                    m1.start_line == m2.start_line
                        && m1.start_col == m2.start_col
                        && m1.text == m2.text
                })
            })
            .collect()
    }
}
