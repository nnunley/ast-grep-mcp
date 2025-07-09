use crate::types::*;
use rmcp::model::{CallToolResult, Content};
use serde_json;

pub struct ResponseFormatter;

impl ResponseFormatter {
    /// Create a formatted response with both JSON data and human-readable text
    pub fn create_formatted_response<T>(
        result: &T,
        summary: String,
    ) -> Result<CallToolResult, Box<dyn std::error::Error + Send + Sync>>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(result)?;

        let contents = vec![Content::text(summary), Content::json(json_value)?];

        Ok(CallToolResult::success(contents))
    }

    /// Create a lightweight formatted response for large results (summary only)
    pub fn create_lightweight_response_for_file_search(
        result: &FileSearchResult,
        summary: String,
    ) -> Result<CallToolResult, Box<dyn std::error::Error + Send + Sync>> {
        // For large results, only return the summary to avoid token limits
        // Include minimal JSON data with cursor information for pagination
        let mut minimal_json = serde_json::json!({
            "summary": "Full JSON data omitted due to size - use smaller max_results or pagination",
            "total_files_found": result.total_files_found,
            "files_in_response": result.matches.len(),
            "has_more": result.next_cursor.as_ref().is_some_and(|c| !c.is_complete)
        });

        if let Some(cursor) = &result.next_cursor {
            minimal_json["next_cursor"] = serde_json::json!({
                "last_file_path": cursor.last_file_path,
                "is_complete": cursor.is_complete
            });
        }

        let contents = vec![Content::text(summary), Content::json(minimal_json)?];

        Ok(CallToolResult::success(contents))
    }

    /// Create a lightweight formatted response for large results (summary only)
    pub fn create_lightweight_response<T>(
        _result: &T,
        summary: String,
    ) -> Result<CallToolResult, Box<dyn std::error::Error + Send + Sync>>
    where
        T: serde::Serialize,
    {
        // For large results, only return the summary to avoid token limits
        // Include minimal JSON data for pagination cursors
        let minimal_json = serde_json::json!({
            "summary": "Full JSON data omitted due to size - use smaller max_results or pagination",
            "has_more": true
        });

        let contents = vec![Content::text(summary), Content::json(minimal_json)?];

        Ok(CallToolResult::success(contents))
    }

    /// Format a file search result with a readable summary
    pub fn format_file_search_result(result: &FileSearchResult) -> String {
        let total_matches: usize = result.matches.iter().map(|f| f.matches.len()).sum();

        if result.matches.is_empty() {
            return "🔍 **No matches found**\n\nNo files matched the search pattern.".to_string();
        }

        let mut summary = format!(
            "🔍 **Search Results**\n\n📁 **Files**: {} files\n🎯 **Matches**: {} total matches\n",
            result.matches.len(),
            total_matches
        );

        // Add file details
        for (i, file_match) in result.matches.iter().enumerate() {
            summary.push_str(&format!(
                "\n📄 **File {}**: `{}`\n",
                i + 1,
                file_match.file_path
            ));

            if file_match.matches.is_empty() {
                summary.push_str("   ❌ No matches in this file\n");
            } else {
                summary.push_str(&format!(
                    "   ✅ {} matches found:\n",
                    file_match.matches.len()
                ));

                // Show first few matches
                for (j, match_result) in file_match.matches.iter().take(3).enumerate() {
                    summary.push_str(&format!(
                        "   {}. **Line {}-{}**: `{}`\n",
                        j + 1,
                        match_result.start_line,
                        match_result.end_line,
                        match_result.text.trim()
                    ));
                }

                if file_match.matches.len() > 3 {
                    summary.push_str(&format!(
                        "   ... and {} more matches\n",
                        file_match.matches.len() - 3
                    ));
                }
            }
        }

        // Add pagination info
        if let Some(cursor) = &result.next_cursor {
            if !cursor.is_complete {
                summary.push_str(&format!(
                    "\n📄 **More results available** - use cursor for pagination:\n```json\n{{\n  \"cursor\": {{\n    \"last_file_path\": \"{}\",\n    \"is_complete\": false\n  }}\n}}\n```",
                    cursor.last_file_path
                ));
            } else {
                summary.push_str("\n✅ **Search complete** - all results have been returned");
            }
        }

        summary
    }

    /// Format a search result with a readable summary
    pub fn format_search_result(result: &SearchResult) -> String {
        if result.matches.is_empty() {
            return "🔍 **No matches found**\n\nThe pattern did not match anything in the provided code.".to_string();
        }

        let mut summary = format!(
            "🔍 **Search Results**\n\n🎯 **Matches**: {} found\n",
            result.matches.len()
        );

        for (i, match_result) in result.matches.iter().enumerate() {
            summary.push_str(&format!(
                "\n{}. **Line {}-{}** (Col {}-{}):\n```\n{}\n```\n",
                i + 1,
                match_result.start_line,
                match_result.end_line,
                match_result.start_col,
                match_result.end_col,
                match_result.text.trim()
            ));

            // Show captured variables if any
            if !match_result.vars.is_empty() {
                summary.push_str("   **Variables captured**:\n");
                for (var, value) in &match_result.vars {
                    summary.push_str(&format!("   - `{var}` = `{value}`\n"));
                }
            }
        }

        summary
    }

    /// Format a replace result with a readable summary
    pub fn format_replace_result(result: &ReplaceResult) -> String {
        if result.changes.is_empty() {
            return "🔄 **No changes made**\n\nThe pattern did not match anything in the provided code.".to_string();
        }

        let mut summary = format!(
            "🔄 **Replace Results**\n\n✅ **Changes**: {} replacements made\n",
            result.changes.len()
        );

        for (i, change) in result.changes.iter().enumerate() {
            summary.push_str(&format!(
                "\n{}. **Line {}-{}**:\n",
                i + 1,
                change.start_line,
                change.end_line
            ));
            summary.push_str(&format!("   **Before**: `{}`\n", change.old_text.trim()));
            summary.push_str(&format!("   **After**: `{}`\n", change.new_text.trim()));
        }

        summary
    }

    /// Format a file replace result with a readable summary
    pub fn format_file_replace_result(result: &FileReplaceResult) -> String {
        if result.total_changes == 0 {
            return "🔄 **No changes made**\n\nThe pattern did not match anything in the searched files.".to_string();
        }

        let mut summary = format!(
            "🔄 **File Replace Results**\n\n📁 **Files modified**: {}\n✅ **Total changes**: {}\n",
            result.files_with_changes, result.total_changes
        );

        // Check if we're in summary mode (only summary_results populated)
        if !result.summary_results.is_empty() && result.file_results.is_empty() {
            summary.push_str("\n📊 **Summary mode** - detailed changes not shown");

            // Show summary results
            for (i, summary_result) in result.summary_results.iter().take(5).enumerate() {
                if summary_result.total_changes == 0 {
                    continue;
                }
                summary.push_str(&format!(
                    "\n📄 **File {}**: `{}`\n   ✅ {} changes on {} lines\n",
                    i + 1,
                    summary_result.file_path,
                    summary_result.total_changes,
                    summary_result.lines_changed
                ));
            }

            if result.summary_results.len() > 5 {
                summary.push_str(&format!(
                    "\n... and {} more files",
                    result.summary_results.len() - 5
                ));
            }

            return summary;
        }

        // Show file details
        for (i, file_result) in result.file_results.iter().take(5).enumerate() {
            if file_result.total_changes == 0 {
                continue;
            }

            summary.push_str(&format!(
                "\n📄 **File {}**: `{}`\n",
                i + 1,
                file_result.file_path
            ));
            summary.push_str(&format!(
                "   ✅ {} changes made:\n",
                file_result.total_changes
            ));

            // Show first few changes
            for (j, change) in file_result.changes.iter().take(2).enumerate() {
                summary.push_str(&format!(
                    "   {}. **Line {}**: `{}` → `{}`\n",
                    j + 1,
                    change.start_line,
                    change.old_text.trim(),
                    change.new_text.trim()
                ));
            }

            if file_result.changes.len() > 2 {
                summary.push_str(&format!(
                    "   ... and {} more changes\n",
                    file_result.changes.len() - 2
                ));
            }
        }

        if result.file_results.len() > 5 {
            summary.push_str(&format!(
                "\n... and {} more files modified",
                result.file_results.len() - 5
            ));
        }

        summary
    }

    /// Format a list languages result with a readable summary
    pub fn format_list_languages_result(result: &ListLanguagesResult) -> String {
        format!(
            "🔤 **Supported Languages**\n\n📝 **Total**: {} languages supported\n\n**Languages**:\n{}",
            result.languages.len(),
            result
                .languages
                .iter()
                .enumerate()
                .map(|(i, lang)| format!("{}. `{}`", i + 1, lang))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    /// Format a documentation result with a readable summary
    pub fn format_documentation_result(result: &DocumentationResult) -> String {
        format!("📚 **AST-Grep Documentation**\n\n{}", result.content)
    }

    /// Format a generate AST result with a readable summary
    pub fn format_generate_ast_result(result: &GenerateAstResult) -> String {
        let mut summary = format!(
            "🌳 **AST Generation Results**\n\n📝 **Language**: {}\n📏 **Code length**: {} characters\n🏷️ **Node types**: {} available\n",
            result.language,
            result.code_length,
            result.node_kinds.len()
        );

        summary.push_str("\n**Available Node Types**:\n");
        for (i, kind) in result.node_kinds.iter().take(10).enumerate() {
            summary.push_str(&format!("{}. `{}`\n", i + 1, kind));
        }

        if result.node_kinds.len() > 10 {
            summary.push_str(&format!(
                "... and {} more node types\n",
                result.node_kinds.len() - 10
            ));
        }

        summary.push_str(&format!("\n**AST Structure**:\n```\n{}\n```", result.ast));

        summary
    }

    /// Format rule validation result with a readable summary
    pub fn format_rule_validate_result(result: &crate::rules::RuleValidateResult) -> String {
        if result.valid {
            let mut summary =
                "✅ **Rule Validation Successful**\n\n🎯 Rule syntax is valid and ready to use."
                    .to_string();

            if let Some(test_results) = &result.test_results {
                summary.push_str(&format!(
                    "\n\n🧪 **Test Results**:\n📊 **Matches found**: {}\n",
                    test_results.matches_found
                ));

                if !test_results.sample_matches.is_empty() {
                    summary.push_str("\n**Sample matches**:\n");
                    for (i, sample) in test_results.sample_matches.iter().take(3).enumerate() {
                        summary.push_str(&format!("{}. `{}`\n", i + 1, sample.trim()));
                    }
                }
            }

            summary
        } else {
            let mut summary =
                "❌ **Rule Validation Failed**\n\n🚨 The rule configuration has errors:\n\n"
                    .to_string();

            for (i, error) in result.errors.iter().enumerate() {
                summary.push_str(&format!("{}. {}\n", i + 1, error));
            }

            summary.push_str(
                "\n💡 **Tip**: Check the rule syntax and ensure all required fields are present.",
            );
            summary
        }
    }

    /// Format rule management results (create, list, get, delete)
    pub fn format_rule_management_result(operation: &str, success: bool, details: &str) -> String {
        let emoji = if success { "✅" } else { "❌" };
        let status = if success { "Successful" } else { "Failed" };

        format!(
            "{} **Rule {} {}**\n\n{}",
            emoji,
            operation.to_title_case(),
            status,
            details
        )
    }

    /// Format suggest patterns result with a readable summary
    pub fn format_suggest_patterns_result(result: &SuggestPatternsResult) -> String {
        if result.suggestions.is_empty() {
            return "🔍 **No pattern suggestions found**\n\nNo patterns could be suggested for the provided code examples.".to_string();
        }

        let mut summary = format!(
            "🔍 **Pattern Suggestions**\n\n📊 **Total**: {} suggestions for {} code\n",
            result.suggestions.len(),
            result.language
        );

        // Add pattern details
        for (i, suggestion) in result.suggestions.iter().enumerate() {
            summary.push_str(&format!(
                "\n**Pattern {}**: `{}`\n",
                i + 1,
                suggestion.pattern
            ));

            summary.push_str(&format!(
                "   📈 **Confidence**: {:.1}%\n",
                suggestion.confidence * 100.0
            ));

            summary.push_str(&format!(
                "   🎯 **Specificity**: {:?}\n",
                suggestion.specificity
            ));

            if !suggestion.explanation.is_empty() {
                summary.push_str(&format!(
                    "   📝 **Explanation**: {}\n",
                    suggestion.explanation
                ));
            }

            if !suggestion.matching_examples.is_empty() {
                summary.push_str(&format!(
                    "   ✅ **Matches examples**: {:?}\n",
                    suggestion.matching_examples
                ));
            }
        }

        summary
    }
}

// Helper trait to convert strings to title case
trait ToTitleCase {
    fn to_title_case(&self) -> String;
}

impl ToTitleCase for str {
    fn to_title_case(&self) -> String {
        let mut result = String::new();
        let mut capitalize_next = true;

        for ch in self.chars() {
            if ch.is_alphabetic() {
                if capitalize_next {
                    result.push(ch.to_uppercase().next().unwrap_or(ch));
                    capitalize_next = false;
                } else {
                    result.push(ch.to_lowercase().next().unwrap_or(ch));
                }
            } else {
                result.push(ch);
                capitalize_next = ch.is_whitespace() || ch == '_' || ch == '-';
            }
        }

        result
    }
}

impl ResponseFormatter {
    /// Format a debug pattern result into a human-readable summary
    pub fn format_debug_pattern_result(result: &DebugPatternResult) -> String {
        let mut summary = String::new();

        summary.push_str("🔍 Pattern Debug Analysis\n");
        summary.push_str(&format!("Pattern: {}\n", result.pattern));
        summary.push_str(&format!("Language: {}\n", result.language));
        summary.push_str(&format!("Format: {:?}\n\n", result.format));

        summary.push_str("📊 Analysis Results:\n");
        summary.push_str(&result.debug_info);
        summary.push('\n');

        summary.push_str("💡 Explanation:\n");
        summary.push_str(&result.explanation);
        summary.push('\n');

        if let Some(ref matches) = result.sample_matches {
            summary.push_str("\n✅ Sample Code Testing:\n");
            if matches.is_empty() {
                summary.push_str("  No matches found in sample code\n");
            } else {
                summary.push_str(&format!(
                    "  Found {} match(es) in sample code:\n",
                    matches.len()
                ));
                for (i, match_result) in matches.iter().take(3).enumerate() {
                    summary.push_str(&format!("  {}. {}\n", i + 1, match_result.text));
                }
                if matches.len() > 3 {
                    summary.push_str(&format!("  ... and {} more matches\n", matches.len() - 3));
                }
            }
        }

        summary
    }

    /// Format a debug AST result into a human-readable summary
    pub fn format_debug_ast_result(result: &DebugAstResult) -> String {
        let mut summary = String::new();

        summary.push_str("🌳 AST Debug Analysis\n");
        summary.push_str(&format!("Language: {}\n", result.language));
        summary.push_str(&format!("Format: {:?}\n", result.format));
        summary.push_str(&format!(
            "Code Length: {} characters\n\n",
            result.code_length
        ));

        summary.push_str("📊 Tree Statistics:\n");
        summary.push_str(&format!(
            "  Total Nodes: {}\n",
            result.tree_stats.total_nodes
        ));
        summary.push_str(&format!("  Leaf Nodes: {}\n", result.tree_stats.leaf_nodes));
        summary.push_str(&format!("  Max Depth: {}\n", result.tree_stats.max_depth));
        summary.push_str(&format!(
            "  Error Nodes: {}\n\n",
            result.tree_stats.error_nodes
        ));

        summary.push_str("🏷️ Node Types Found:\n");
        if result.node_kinds.is_empty() {
            summary.push_str("  No node types detected\n");
        } else {
            for (i, node_kind) in result.node_kinds.iter().take(10).enumerate() {
                summary.push_str(&format!("  {}. {}\n", i + 1, node_kind));
            }
            if result.node_kinds.len() > 10 {
                summary.push_str(&format!(
                    "  ... and {} more node types\n",
                    result.node_kinds.len() - 10
                ));
            }
        }

        summary.push_str("\n🌲 Syntax Tree:\n");
        // Show first few lines of the tree
        let tree_lines: Vec<&str> = result.tree.lines().take(20).collect();
        for line in tree_lines {
            summary.push_str(&format!("  {line}\n"));
        }
        if result.tree.lines().count() > 20 {
            summary.push_str("  ... (tree truncated, see full JSON response for complete tree)\n");
        }

        summary
    }
}
