use super::ast::{PatternRule, Rule};
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

    /// Evaluate a Rule enum against code
    pub fn evaluate_rule(
        &self,
        rule: &Rule,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        match rule {
            Rule::Pattern(pattern_rule) => {
                self.evaluate_pattern_rule_enum(pattern_rule, code, lang)
            }
            Rule::Kind(kind) => self.evaluate_kind_rule(kind, code, lang),
            Rule::Regex(regex) => self.evaluate_regex_rule(regex, code, lang),
            Rule::Matches(_matches) => {
                // TODO: Implement matches rule - needs access to rule storage
                Err(ServiceError::ParserError(
                    "Matches rule evaluation not yet implemented".into(),
                ))
            }
            Rule::All(rules) => self.evaluate_all_rule_enum(rules, code, lang),
            Rule::Any(rules) => self.evaluate_any_rule_enum(rules, code, lang),
            Rule::Not(rule) => self.evaluate_not_rule_enum(rule, code, lang),
            Rule::Inside { rule, inside_of } => {
                self.evaluate_inside_rule_enum(rule, inside_of, code, lang)
            }
            Rule::Has { rule, contains } => self.evaluate_has_rule_enum(rule, contains, code, lang),
            Rule::Follows { rule, after } => {
                self.evaluate_follows_rule_enum(rule, after, code, lang)
            }
            Rule::Precedes { rule, before } => {
                self.evaluate_precedes_rule_enum(rule, before, code, lang)
            }
        }
    }

    pub fn evaluate_rule_against_code(
        &self,
        rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Convert RuleObject to Rule and use the new evaluation method
        let rule_enum = Rule::from(rule.clone());
        self.evaluate_rule(&rule_enum, code, lang)
    }

    pub fn evaluate_rule_against_code_old(
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
        } else if let Some(inside_rule) = &rule.inside {
            // Inside relational rule - match nodes inside another pattern
            self.evaluate_inside_rule(inside_rule, code, lang)
        } else if let Some(has_rule) = &rule.has {
            // Has relational rule - match nodes that contain another pattern
            self.evaluate_has_rule(has_rule, code, lang)
        } else if let Some(follows_rule) = &rule.follows {
            // Follows relational rule - match nodes that follow another pattern
            self.evaluate_follows_rule(follows_rule, code, lang)
        } else if let Some(precedes_rule) = &rule.precedes {
            // Precedes relational rule - match nodes that precede another pattern
            self.evaluate_precedes_rule(precedes_rule, code, lang)
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

        // For All rules, we need to find nodes that satisfy ALL conditions
        // This means finding nodes that match the primary rule and also contain/satisfy other rules

        // Start with matches from the first rule as potential candidates
        let primary_matches = self.evaluate_rule_against_code(&all_rules[0], code, lang)?;

        // Filter candidates that also satisfy all other rules
        let mut result_matches = Vec::new();

        for primary_match in primary_matches {
            let mut satisfies_all = true;

            // Check if this match satisfies all other rules
            for rule in &all_rules[1..] {
                let rule_matches = self.evaluate_rule_against_code(rule, code, lang)?;

                // Check if any rule match is contained within or overlaps with the primary match
                let satisfies_rule = rule_matches.iter().any(|rule_match| {
                    self.is_match_contained_in(&primary_match, rule_match)
                        || self.matches_overlap(&primary_match, rule_match)
                });

                if !satisfies_rule {
                    satisfies_all = false;
                    break;
                }
            }

            if satisfies_all {
                result_matches.push(primary_match);
            }
        }

        Ok(result_matches)
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
        // We exclude both exact matches and nodes that overlap with NOT match text
        let filtered_matches: Vec<MatchResult> = all_nodes
            .into_iter()
            .filter(|node| {
                !not_matches.iter().any(|not_match| {
                    // Exact match (same position and text)
                    (node.start_line == not_match.start_line
                        && node.start_col == not_match.start_col
                        && node.text == not_match.text)
                    // Or NOT match contains the node text (node is a substring of what we want to exclude)
                    || not_match.text.contains(&node.text)
                    // Or node contains the NOT match text (node is a parent of what we want to exclude)
                    || node.text.contains(&not_match.text)
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
        let ast = AstGrep::new(code, lang);

        // Use a catch-all pattern to find all nodes, then filter by kind
        let catch_all_pattern = "$_";
        match self.get_or_create_pattern(catch_all_pattern, lang) {
            Ok(pattern) => {
                let all_matches: Vec<MatchResult> = ast
                    .root()
                    .find_all(pattern)
                    .filter(|node| node.get_node().kind() == kind)
                    .map(|node| MatchResult::from_node_match(&node))
                    .collect();

                Ok(all_matches)
            }
            Err(_) => {
                // If that fails, return empty results
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
                    context_before: None,
                    context_after: None,
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

    #[allow(dead_code)]
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

    fn is_match_contained_in(&self, container: &MatchResult, contained: &MatchResult) -> bool {
        // Check if 'contained' is within the bounds of 'container'
        if container.start_line < contained.start_line && contained.end_line <= container.end_line {
            return true;
        }
        if container.start_line == contained.start_line && contained.end_line <= container.end_line
        {
            return container.start_col <= contained.start_col;
        }
        if container.start_line < contained.start_line && contained.end_line == container.end_line {
            return contained.end_col <= container.end_col;
        }
        if container.start_line == contained.start_line && contained.end_line == container.end_line
        {
            return container.start_col <= contained.start_col
                && contained.end_col <= container.end_col;
        }
        false
    }

    fn matches_overlap(&self, match1: &MatchResult, match2: &MatchResult) -> bool {
        // Check if two matches overlap in any way
        !(match1.end_line < match2.start_line
            || match2.end_line < match1.start_line
            || (match1.end_line == match2.start_line && match1.end_col < match2.start_col)
            || (match2.end_line == match1.start_line && match2.end_col < match1.start_col))
    }

    fn evaluate_inside_rule(
        &self,
        inside_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Convert to Rule enum and use the new implementation
        let rule_enum = Rule::from(inside_rule.clone());
        // For inside rules, we need to extract the base and container parts
        // This is a simplified implementation - real ast-grep has more complex syntax
        self.evaluate_rule(&rule_enum, code, lang)
    }

    fn evaluate_has_rule(
        &self,
        has_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Convert to Rule enum and use the new implementation
        let rule_enum = Rule::from(has_rule.clone());
        self.evaluate_rule(&rule_enum, code, lang)
    }

    fn evaluate_follows_rule(
        &self,
        follows_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Convert to Rule enum and use the new implementation
        let rule_enum = Rule::from(follows_rule.clone());
        self.evaluate_rule(&rule_enum, code, lang)
    }

    fn evaluate_precedes_rule(
        &self,
        precedes_rule: &RuleObject,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Convert to Rule enum and use the new implementation
        let rule_enum = Rule::from(precedes_rule.clone());
        self.evaluate_rule(&rule_enum, code, lang)
    }

    // Enum-based evaluation methods

    fn evaluate_pattern_rule_enum(
        &self,
        pattern_rule: &PatternRule,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let pattern_str = match pattern_rule {
            PatternRule::Simple { pattern } => pattern.clone(),
            PatternRule::Advanced { pattern, .. } => pattern.clone(),
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

    fn evaluate_all_rule_enum(
        &self,
        all_rules: &[Rule],
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        if all_rules.is_empty() {
            return Ok(vec![]);
        }

        // Use fold to reduce rules into a filtered set of candidates
        all_rules
            .iter()
            .enumerate()
            .try_fold(vec![], |candidates, (i, rule)| {
                if i == 0 {
                    // First rule: get initial candidates
                    self.evaluate_rule(rule, code, lang)
                } else {
                    // Subsequent rules: filter existing candidates
                    // Early exit optimization: if no candidates, short-circuit
                    if candidates.is_empty() {
                        Ok(vec![])
                    } else {
                        self.filter_candidates_by_rule(candidates, rule, code, lang)
                    }
                }
            })
    }

    /// Filter candidates based on a rule constraint
    fn filter_candidates_by_rule(
        &self,
        candidates: Vec<MatchResult>,
        rule: &Rule,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        match rule {
            // For Has rules: keep candidates that contain the pattern
            Rule::Has { rule: _, contains } => {
                let mut filtered = Vec::new();
                let contains_matches = self.evaluate_rule(contains, code, lang)?;

                for candidate in candidates {
                    // Check if any contains_match is within this candidate
                    let has_match = contains_matches
                        .iter()
                        .any(|m| self.is_match_contained_in(&candidate, m));

                    if has_match {
                        filtered.push(candidate);
                    }
                }
                Ok(filtered)
            }

            // For Inside rules: keep candidates that are inside the pattern
            Rule::Inside { rule: _, inside_of } => {
                let mut filtered = Vec::new();
                let container_matches = self.evaluate_rule(inside_of, code, lang)?;

                for candidate in candidates {
                    // Check if this candidate is inside any container
                    let is_inside = container_matches
                        .iter()
                        .any(|container| self.is_match_contained_in(container, &candidate));

                    if is_inside {
                        filtered.push(candidate);
                    }
                }
                Ok(filtered)
            }

            // For Not rules: keep candidates that don't match the sub-rule
            Rule::Not(not_rule) => {
                match not_rule.as_ref() {
                    // Special handling for "not has" pattern
                    Rule::Has { rule: _, contains } => {
                        let mut filtered = Vec::new();
                        let contains_matches = self.evaluate_rule(contains, code, lang)?;

                        for candidate in candidates {
                            // Check if any contains_match is within this candidate
                            let has_match = contains_matches
                                .iter()
                                .any(|m| self.is_match_contained_in(&candidate, m));

                            // Keep candidates that DON'T have the match
                            if !has_match {
                                filtered.push(candidate);
                            }
                        }
                        Ok(filtered)
                    }
                    _ => {
                        // For other not rules, evaluate normally
                        let not_matches = self.evaluate_rule(not_rule, code, lang)?;
                        Ok(candidates
                            .into_iter()
                            .filter(|candidate| {
                                !not_matches
                                    .iter()
                                    .any(|m| self.matches_overlap(candidate, m))
                            })
                            .collect())
                    }
                }
            }

            // For Regex rules: check if candidate text matches the regex
            Rule::Regex(regex_pattern) => {
                let regex = Regex::new(regex_pattern)?;
                Ok(candidates
                    .into_iter()
                    .filter(|candidate| regex.is_match(&candidate.text))
                    .collect())
            }

            // For other rules: use overlap logic
            _ => {
                let rule_matches = self.evaluate_rule(rule, code, lang)?;
                Ok(candidates
                    .into_iter()
                    .filter(|candidate| {
                        rule_matches
                            .iter()
                            .any(|m| self.matches_overlap(candidate, m))
                    })
                    .collect())
            }
        }
    }

    fn evaluate_any_rule_enum(
        &self,
        any_rules: &[Rule],
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        let mut all_matches = Vec::new();

        // Collect matches from all rules
        for rule in any_rules {
            let mut rule_matches = self.evaluate_rule(rule, code, lang)?;
            all_matches.append(&mut rule_matches);
        }

        // Remove duplicates based on text content and position
        all_matches.sort_by_key(|m| (m.start_line, m.start_col, m.text.clone()));
        all_matches.dedup_by_key(|m| (m.start_line, m.start_col, m.text.clone()));

        Ok(all_matches)
    }

    fn evaluate_not_rule_enum(
        &self,
        not_rule: &Rule,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Get all potential nodes by using a catch-all pattern
        let catch_all_pattern = "$_";
        let ast = AstGrep::new(code, lang);
        let pattern = self.get_or_create_pattern(catch_all_pattern, lang)?;

        let all_nodes: Vec<MatchResult> = ast
            .root()
            .find_all(pattern)
            .map(|node| MatchResult::from_node_match(&node))
            .collect();

        // Get matches from the NOT rule
        let not_matches = self.evaluate_rule(not_rule, code, lang)?;

        // Return nodes that don't match the NOT rule
        // We exclude both exact matches and nodes that overlap with NOT match text
        let filtered_matches: Vec<MatchResult> = all_nodes
            .into_iter()
            .filter(|node| {
                !not_matches.iter().any(|not_match| {
                    // Exact match (same position and text)
                    (node.start_line == not_match.start_line
                        && node.start_col == not_match.start_col
                        && node.text == not_match.text)
                    // Or NOT match contains the node text (node is a substring of what we want to exclude)
                    || not_match.text.contains(&node.text)
                    // Or node contains the NOT match text (node is a parent of what we want to exclude)
                    || node.text.contains(&not_match.text)
                })
            })
            .collect();

        Ok(filtered_matches)
    }

    fn evaluate_inside_rule_enum(
        &self,
        rule: &Rule,
        inside_of: &Rule,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // First, find all nodes that match the container rule
        let container_matches = self.evaluate_rule(inside_of, code, lang)?;

        // For each container, find nodes inside it that match the main rule
        let mut results = Vec::new();
        let ast = AstGrep::new(code, lang);

        for container in &container_matches {
            // Find the AST node for this container match
            // We need to search for nodes within the container's range
            if let Some(pattern) = rule.extract_pattern() {
                let pattern_obj = self.get_or_create_pattern(&pattern, lang)?;

                // Search within the container's text range
                let container_start = container.start_line;
                let container_end = container.end_line;

                // Find all matches of the pattern
                let all_matches: Vec<MatchResult> = ast
                    .root()
                    .find_all(pattern_obj)
                    .filter_map(|node| {
                        let match_result = MatchResult::from_node_match(&node);
                        // Check if this match is within the container's range
                        if match_result.start_line >= container_start
                            && match_result.end_line <= container_end
                        {
                            Some(match_result)
                        } else {
                            None
                        }
                    })
                    .collect();

                results.extend(all_matches);
            }
        }

        // Remove duplicates
        results.sort_by_key(|m| (m.start_line, m.start_col, m.text.clone()));
        results.dedup_by_key(|m| (m.start_line, m.start_col, m.text.clone()));

        Ok(results)
    }

    fn evaluate_has_rule_enum(
        &self,
        rule: &Rule,
        contains: &Rule,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // First, find all nodes that match the main rule
        let main_matches = self.evaluate_rule(rule, code, lang)?;

        // Then find all nodes that match the contains rule
        let contained_matches = self.evaluate_rule(contains, code, lang)?;

        // Filter main matches to only those that contain at least one of the contained matches
        let filtered_matches: Vec<MatchResult> = main_matches
            .into_iter()
            .filter(|main_match| {
                // Check if any contained match is within this main match's range
                contained_matches.iter().any(|contained| {
                    self.is_match_contained_in(main_match, contained)
                        || self.matches_overlap(main_match, contained)
                })
            })
            .collect();

        Ok(filtered_matches)
    }

    fn evaluate_follows_rule_enum(
        &self,
        rule: &Rule,
        after: &Rule,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Find all nodes that match the main rule
        let main_matches = self.evaluate_rule(rule, code, lang)?;

        // Find all nodes that match the "after" rule
        let after_matches = self.evaluate_rule(after, code, lang)?;

        // Filter main matches to only those that follow at least one "after" match
        let filtered_matches: Vec<MatchResult> = main_matches
            .into_iter()
            .filter(|main_match| {
                // Check if this main match follows any "after" match
                after_matches.iter().any(|after_match| {
                    // The main match follows if it starts after the "after" match ends
                    main_match.start_line > after_match.end_line
                        || (main_match.start_line == after_match.end_line
                            && main_match.start_col > after_match.end_col)
                })
            })
            .collect();

        Ok(filtered_matches)
    }

    fn evaluate_precedes_rule_enum(
        &self,
        rule: &Rule,
        before: &Rule,
        code: &str,
        lang: Language,
    ) -> Result<Vec<MatchResult>, ServiceError> {
        // Find all nodes that match the main rule
        let main_matches = self.evaluate_rule(rule, code, lang)?;

        // Find all nodes that match the "before" rule
        let before_matches = self.evaluate_rule(before, code, lang)?;

        // Filter main matches to only those that precede at least one "before" match
        let filtered_matches: Vec<MatchResult> = main_matches
            .into_iter()
            .filter(|main_match| {
                // Check if this main match precedes any "before" match
                before_matches.iter().any(|before_match| {
                    // The main match precedes if it ends before the "before" match starts
                    main_match.end_line < before_match.start_line
                        || (main_match.end_line == before_match.start_line
                            && main_match.end_col < before_match.start_col)
                })
            })
            .collect();

        Ok(filtered_matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ast::{PatternRule, Rule};

    fn create_test_evaluator() -> RuleEvaluator {
        RuleEvaluator::new()
    }

    #[test]
    fn test_pattern_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = r#"
function greet() {
    console.log("Hello");
    console.error("Error");
}
"#;
        let lang = Language::JavaScript;

        let rule = Rule::Pattern(PatternRule::Simple {
            pattern: "console.log($VAR)".to_string(),
        });

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].text.contains("console.log"));
    }

    #[test]
    fn test_kind_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = "function test() { return 42; }";
        let lang = Language::JavaScript;

        let rule = Rule::Kind("function_declaration".to_string());

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].text.contains("function test"));
    }

    #[test]
    fn test_regex_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = r#"
// TODO: Fix this
// FIXME: Bug here
console.log("Normal comment");
"#;
        let lang = Language::JavaScript;

        let rule = Rule::Regex("TODO|FIXME".to_string());

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_all_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = r#"
function greet() {
    console.log("Hello");
}
function test() {
    alert("Test");
}
"#;
        let lang = Language::JavaScript;

        // Test All rule with Tree-sitter kind and pattern
        let rule = Rule::All(vec![
            Rule::Kind("function_declaration".to_string()),
            Rule::Pattern(PatternRule::Simple {
                pattern: "console.log($VAR)".to_string(),
            }),
        ]);

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        // Should find functions that both are function_declarations AND contain console.log
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_any_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = r#"
console.log("Hello");
console.error("Error");
alert("Alert");
"#;
        let lang = Language::JavaScript;

        let rule = Rule::Any(vec![
            Rule::Pattern(PatternRule::Simple {
                pattern: "console.log($VAR)".to_string(),
            }),
            Rule::Pattern(PatternRule::Simple {
                pattern: "console.error($VAR)".to_string(),
            }),
        ]);

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        assert_eq!(result.len(), 2); // Should find both console.log and console.error
    }

    #[test]
    fn test_not_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = r#"
console.log("Hello");
alert("Alert");
"#;
        let lang = Language::JavaScript;

        let rule = Rule::Not(Box::new(Rule::Pattern(PatternRule::Simple {
            pattern: "console.log($VAR)".to_string(),
        })));

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        // Should find nodes that are NOT console.log calls
        assert!(!result.is_empty());
        assert!(!result.iter().any(|m| m.text.contains("console.log")));
    }

    #[test]
    fn test_has_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = r#"
function greet() {
    console.log("Hello");
}
function test() {
    alert("Test");
}
"#;
        let lang = Language::JavaScript;

        let rule = Rule::Has {
            rule: Box::new(Rule::Kind("function_declaration".to_string())),
            contains: Box::new(Rule::Pattern(PatternRule::Simple {
                pattern: "console.log($VAR)".to_string(),
            })),
        };

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        // Should find function declarations that contain console.log
        assert_eq!(result.len(), 1);
        assert!(result[0].text.contains("greet"));
    }

    #[test]
    fn test_follows_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = r#"
var x = 1;
console.log(x);
var y = 2;
"#;
        let lang = Language::JavaScript;

        let rule = Rule::Follows {
            rule: Box::new(Rule::Pattern(PatternRule::Simple {
                pattern: "console.log($VAR)".to_string(),
            })),
            after: Box::new(Rule::Pattern(PatternRule::Simple {
                pattern: "var $NAME = $VALUE".to_string(),
            })),
        };

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        // Should find console.log that follows var declaration
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_precedes_rule_evaluation() {
        let evaluator = create_test_evaluator();
        let code = r#"
var x = 1;
console.log(x);
var y = 2;
"#;
        let lang = Language::JavaScript;

        let rule = Rule::Precedes {
            rule: Box::new(Rule::Pattern(PatternRule::Simple {
                pattern: "var $NAME = $VALUE".to_string(),
            })),
            before: Box::new(Rule::Pattern(PatternRule::Simple {
                pattern: "console.log($VAR)".to_string(),
            })),
        };

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        // Should find var declaration that precedes console.log
        assert_eq!(result.len(), 1);
        assert!(result[0].text.contains("var x"));
    }

    #[test]
    fn test_pattern_caching() {
        let evaluator = create_test_evaluator();
        let code = "console.log('test');";
        let lang = Language::JavaScript;
        let pattern = "console.log($VAR)";

        // First call should create and cache the pattern
        let result1 = evaluator.get_or_create_pattern(pattern, lang).unwrap();

        // Use the pattern to verify it works
        let rule = Rule::Pattern(PatternRule::Simple {
            pattern: pattern.to_string(),
        });
        let matches1 = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        assert_eq!(matches1.len(), 1);

        // Second call should return from cache
        let result2 = evaluator.get_or_create_pattern(pattern, lang).unwrap();

        // Use the cached pattern to verify it still works
        let matches2 = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        assert_eq!(matches2.len(), 1);

        // Both patterns should be functionally equivalent
        assert_eq!(format!("{result1:?}"), format!("{result2:?}"));
        assert_eq!(matches1.len(), matches2.len());
    }

    #[test]
    fn test_empty_all_rule() {
        let evaluator = create_test_evaluator();
        let code = "console.log('test');";
        let lang = Language::JavaScript;

        let rule = Rule::All(vec![]);

        let result = evaluator.evaluate_rule(&rule, code, lang).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_matches_rule_error() {
        let evaluator = create_test_evaluator();
        let code = "console.log('test');";
        let lang = Language::JavaScript;

        let rule = Rule::Matches("some-rule-id".to_string());

        let result = evaluator.evaluate_rule(&rule, code, lang);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented")
        );
    }

    #[test]
    fn test_intersect_matches() {
        let evaluator = create_test_evaluator();
        let match1 = MatchResult {
            text: "test".to_string(),
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 4,
            vars: std::collections::HashMap::new(),
            context_before: None,
            context_after: None,
        };
        let match2 = MatchResult {
            text: "test".to_string(),
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 4,
            vars: std::collections::HashMap::new(),
            context_before: None,
            context_after: None,
        };
        let match3 = MatchResult {
            text: "other".to_string(),
            start_line: 2,
            end_line: 2,
            start_col: 0,
            end_col: 5,
            vars: std::collections::HashMap::new(),
            context_before: None,
            context_after: None,
        };

        let matches1 = vec![match1.clone(), match3.clone()];
        let matches2 = vec![match2, match1];

        let intersection = evaluator.intersect_matches(matches1, matches2);
        assert_eq!(intersection.len(), 1);
        assert_eq!(intersection[0].text, "test");
    }
}
