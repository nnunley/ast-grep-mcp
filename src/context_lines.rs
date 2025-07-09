use crate::types::MatchResult;

/// Extract context lines from source code for a match result
pub fn extract_context_lines(
    source_code: &str,
    matches: &[MatchResult],
    context_before: Option<usize>,
    context_after: Option<usize>,
    context_lines: Option<usize>,
) -> Vec<MatchResult> {
    let lines: Vec<&str> = source_code.lines().collect();
    let total_lines = lines.len();

    // Determine actual context before and after values
    let (before_count, after_count) = if let Some(context_lines) = context_lines {
        (context_lines, context_lines)
    } else {
        (context_before.unwrap_or(0), context_after.unwrap_or(0))
    };

    matches
        .iter()
        .map(|m| {
            let start_line = m.start_line;
            let end_line = m.end_line;

            // Calculate context range with bounds checking
            let _context_start = if start_line > before_count {
                start_line - before_count - 1 // Convert to 0-based index
            } else {
                0
            };

            let _context_end = std::cmp::min(end_line + after_count, total_lines);

            // Extract context before (lines before the match)
            let context_before_lines = if before_count > 0 && start_line > 1 {
                let actual_start = if start_line > before_count {
                    start_line - before_count - 1 // Convert to 0-based index
                } else {
                    0
                };
                let actual_end = start_line - 1; // Convert to 0-based index

                if actual_start < actual_end {
                    Some(
                        lines[actual_start..actual_end]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    )
                } else {
                    Some(Vec::new())
                }
            } else if before_count > 0 {
                Some(Vec::new())
            } else {
                None
            };

            // Extract context after (lines after the match)
            let context_after_lines = if after_count > 0 && end_line < total_lines {
                let actual_start = end_line; // end_line is 1-based, so convert to 0-based index for line after match
                let actual_end = std::cmp::min(end_line + after_count, total_lines);

                if actual_start < actual_end {
                    Some(
                        lines[actual_start..actual_end]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    )
                } else {
                    Some(Vec::new())
                }
            } else if after_count > 0 {
                Some(Vec::new())
            } else {
                None
            };

            m.clone()
                .with_context(context_before_lines, context_after_lines)
        })
        .collect()
}

/// Add context lines to search results
pub fn add_context_to_search_result(
    source_code: &str,
    result: crate::types::SearchResult,
    context_before: Option<usize>,
    context_after: Option<usize>,
    context_lines: Option<usize>,
) -> crate::types::SearchResult {
    let matches_with_context = extract_context_lines(
        source_code,
        &result.matches,
        context_before,
        context_after,
        context_lines,
    );

    crate::types::SearchResult {
        matches: matches_with_context,
        matches_summary: result.matches_summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MatchResult;
    use std::collections::HashMap;

    #[test]
    fn test_extract_context_lines_basic() {
        let source = "line 1\nline 2\nMATCH\nline 4\nline 5";
        let matches = vec![MatchResult {
            text: "MATCH".to_string(),
            start_line: 3,
            end_line: 3,
            start_col: 0,
            end_col: 5,
            vars: HashMap::new(),
            context_before: None,
            context_after: None,
        }];

        let result = extract_context_lines(source, &matches, Some(1), Some(1), None);

        assert_eq!(result.len(), 1);
        let match_result = &result[0];
        assert_eq!(
            match_result.context_before,
            Some(vec!["line 2".to_string()])
        );
        assert_eq!(match_result.context_after, Some(vec!["line 4".to_string()]));
    }

    #[test]
    fn test_extract_context_lines_at_boundary() {
        let source = "MATCH\nline 2\nline 3";
        let matches = vec![MatchResult {
            text: "MATCH".to_string(),
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 5,
            vars: HashMap::new(),
            context_before: None,
            context_after: None,
        }];

        let result = extract_context_lines(source, &matches, Some(2), Some(1), None);

        assert_eq!(result.len(), 1);
        let match_result = &result[0];
        assert_eq!(match_result.context_before, Some(vec![])); // Empty at file start
        assert_eq!(match_result.context_after, Some(vec!["line 2".to_string()]));
    }

    #[test]
    fn test_context_lines_parameter() {
        let source = "line 1\nline 2\nMATCH\nline 4\nline 5";
        let matches = vec![MatchResult {
            text: "MATCH".to_string(),
            start_line: 3,
            end_line: 3,
            start_col: 0,
            end_col: 5,
            vars: HashMap::new(),
            context_before: None,
            context_after: None,
        }];

        let result = extract_context_lines(source, &matches, None, None, Some(2));

        assert_eq!(result.len(), 1);
        let match_result = &result[0];
        assert_eq!(
            match_result.context_before,
            Some(vec!["line 1".to_string(), "line 2".to_string()])
        );
        assert_eq!(
            match_result.context_after,
            Some(vec!["line 4".to_string(), "line 5".to_string()])
        );
    }
}
