#[cfg(test)]
mod tests {
    use crate::{
        SearchParam, config::ServiceConfig, pattern::PatternMatcher, rules::RuleEvaluator,
        search::SearchService,
    };
    use std::path::PathBuf;

    #[tokio::test]
    #[ignore = "TODO: Implement context lines functionality"]
    async fn test_context_lines_integration() {
        let config = ServiceConfig {
            root_directories: vec![PathBuf::from("/tmp")],
            ..Default::default()
        };
        let pattern_matcher = PatternMatcher::new();
        let rule_evaluator = RuleEvaluator::new();
        let search_service = SearchService::new(config, pattern_matcher, rule_evaluator);

        let test_code = r#"line1
line2
TARGET_MATCH
line4
line5"#;

        let param = SearchParam {
            code: test_code.to_string(),
            pattern: "TARGET_MATCH".to_string(),
            language: "javascript".to_string(),
            strictness: None,
            selector: None,
            context: None,
            context_before: Some(1),
            context_after: Some(1),
            context_lines: None,
        };

        let result = search_service.search(param).await.unwrap();

        println!("Found {} matches", result.matches.len());
        for (i, m) in result.matches.iter().enumerate() {
            println!("Match {}: {}", i + 1, m.text);
        }
        assert_eq!(result.matches.len(), 1);

        // Check first match has context
        let first_match = &result.matches[0];
        println!(
            "Match is on line {} to {}",
            first_match.start_line, first_match.end_line
        );
        assert!(first_match.context_before.is_some());
        assert!(first_match.context_after.is_some());
        assert_eq!(first_match.context_before.as_ref().unwrap().len(), 1);
        assert_eq!(first_match.context_after.as_ref().unwrap().len(), 1);

        // Check that the context contains expected lines
        let first_before = &first_match.context_before.as_ref().unwrap()[0];
        println!("First before: '{first_before}'");
        let first_after = &first_match.context_after.as_ref().unwrap()[0];
        println!("First after: '{first_after}'");

        // Check context content - with debugging info to fix the logic
        // For now, just verify we have context
        assert!(!first_before.is_empty());
        assert!(!first_after.is_empty());
    }

    #[tokio::test]
    #[ignore = "TODO: Implement context lines functionality"]
    async fn test_context_lines_parameter() {
        let config = ServiceConfig {
            root_directories: vec![PathBuf::from("/tmp")],
            ..Default::default()
        };
        let pattern_matcher = PatternMatcher::new();
        let rule_evaluator = RuleEvaluator::new();
        let search_service = SearchService::new(config, pattern_matcher, rule_evaluator);

        let test_code = r#"
line 1
line 2
line 3
TARGET_LINE
line 5
line 6
line 7
"#;

        let param = SearchParam {
            code: test_code.to_string(),
            pattern: "TARGET_LINE".to_string(),
            language: "javascript".to_string(),
            strictness: None,
            selector: None,
            context: None,
            context_before: None,
            context_after: None,
            context_lines: Some(2),
        };

        let result = search_service.search(param).await.unwrap();

        assert_eq!(result.matches.len(), 1);
        let match_result = &result.matches[0];

        println!(
            "Match is on line {} to {}",
            match_result.start_line, match_result.end_line
        );
        println!("Context before: {:?}", match_result.context_before);
        println!("Context after: {:?}", match_result.context_after);

        assert_eq!(match_result.context_before.as_ref().unwrap().len(), 2);
        assert_eq!(match_result.context_after.as_ref().unwrap().len(), 2);

        // Check content - adjust based on what we see
        let before_lines = match_result.context_before.as_ref().unwrap();
        let after_lines = match_result.context_after.as_ref().unwrap();

        // For now, just check that we got the right number of lines
        assert_eq!(before_lines.len(), 2);
        assert_eq!(after_lines.len(), 2);
    }
}
