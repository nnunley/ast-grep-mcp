use ast_grep_mcp::response_formatter::ResponseFormatter;
use ast_grep_mcp::types::*;
use std::collections::HashMap;

#[test]
fn test_search_result_formatting() {
    let mut vars = HashMap::new();
    vars.insert("MSG".to_string(), "'hello'".to_string());

    let result = SearchResult {
        matches: vec![
            MatchResult {
                text: "console.log('hello')".to_string(),
                start_line: 1,
                end_line: 1,
                start_col: 0,
                end_col: 20,
                vars: vars.clone(),
                context_before: None,
                context_after: None,
            },
            MatchResult {
                text: "console.log('world')".to_string(),
                start_line: 2,
                end_line: 2,
                start_col: 0,
                end_col: 20,
                vars: HashMap::new(),
                context_before: None,
                context_after: None,
            },
        ],
        matches_summary: None,
    };

    let summary = ResponseFormatter::format_search_result(&result);

    assert!(summary.contains("🔍 **Search Results**"));
    assert!(summary.contains("🎯 **Matches**: 2 found"));
    assert!(summary.contains("console.log('hello')"));
    assert!(summary.contains("console.log('world')"));
    assert!(summary.contains("**Variables captured**"));
    assert!(summary.contains("`MSG` = `'hello'`"));
}

#[test]
fn test_search_result_no_matches() {
    let result = SearchResult {
        matches: vec![],
        matches_summary: None,
    };

    let summary = ResponseFormatter::format_search_result(&result);

    assert!(summary.contains("🔍 **No matches found**"));
    assert!(summary.contains("The pattern did not match anything"));
}

#[test]
fn test_file_search_result_formatting() {
    let result = FileSearchResult {
        matches: vec![FileMatchResult {
            file_path: "/src/app.js".to_string(),
            file_size_bytes: 1024,
            matches: vec![MatchResult {
                text: "console.log('test')".to_string(),
                start_line: 10,
                end_line: 10,
                start_col: 4,
                end_col: 23,
                vars: HashMap::new(),
                context_before: None,
                context_after: None,
            }],
            file_hash: "abc123".to_string(),
        }],
        next_cursor: None,
        total_files_found: 1,
    };

    let summary = ResponseFormatter::format_file_search_result(&result);

    assert!(summary.contains("🔍 **Search Results**"));
    assert!(summary.contains("📁 **Files**: 1 files"));
    assert!(summary.contains("🎯 **Matches**: 1 total matches"));
    assert!(summary.contains("📄 **File 1**: `/src/app.js`"));
    assert!(summary.contains("✅ 1 matches found"));
    assert!(summary.contains("**Line 10-10**: `console.log('test')`"));
}

#[test]
fn test_file_search_result_no_matches() {
    let result = FileSearchResult {
        matches: vec![],
        next_cursor: None,
        total_files_found: 0,
    };

    let summary = ResponseFormatter::format_file_search_result(&result);

    assert!(summary.contains("🔍 **No matches found**"));
    assert!(summary.contains("No files matched the search pattern"));
}

#[test]
fn test_replace_result_formatting() {
    let result = ReplaceResult {
        new_code: "logger.info('hello'); logger.info('world');".to_string(),
        changes: vec![
            ChangeResult {
                start_line: 1,
                end_line: 1,
                start_col: 0,
                end_col: 20,
                old_text: "console.log('hello')".to_string(),
                new_text: "logger.info('hello')".to_string(),
            },
            ChangeResult {
                start_line: 1,
                end_line: 1,
                start_col: 22,
                end_col: 42,
                old_text: "console.log('world')".to_string(),
                new_text: "logger.info('world')".to_string(),
            },
        ],
    };

    let summary = ResponseFormatter::format_replace_result(&result);

    assert!(summary.contains("🔄 **Replace Results**"));
    assert!(summary.contains("✅ **Changes**: 2 replacements made"));
    assert!(summary.contains("**Before**: `console.log('hello')`"));
    assert!(summary.contains("**After**: `logger.info('hello')`"));
    assert!(summary.contains("**Before**: `console.log('world')`"));
    assert!(summary.contains("**After**: `logger.info('world')`"));
}

#[test]
fn test_list_languages_result_formatting() {
    let result = ListLanguagesResult {
        languages: vec![
            "javascript".to_string(),
            "typescript".to_string(),
            "rust".to_string(),
            "python".to_string(),
        ],
    };

    let summary = ResponseFormatter::format_list_languages_result(&result);

    assert!(summary.contains("🔤 **Supported Languages**"));
    assert!(summary.contains("📝 **Total**: 4 languages supported"));
    assert!(summary.contains("1. `javascript`"));
    assert!(summary.contains("2. `typescript`"));
    assert!(summary.contains("3. `rust`"));
    assert!(summary.contains("4. `python`"));
}

#[test]
fn test_generate_ast_result_formatting() {
    let result = GenerateAstResult {
        language: "javascript".to_string(),
        code_length: 42,
        node_kinds: vec![
            "program".to_string(),
            "function_declaration".to_string(),
            "identifier".to_string(),
            "statement_block".to_string(),
        ],
        ast: "(program (function_declaration name: (identifier) body: (statement_block)))"
            .to_string(),
    };

    let summary = ResponseFormatter::format_generate_ast_result(&result);

    assert!(summary.contains("🌳 **AST Generation Results**"));
    assert!(summary.contains("📝 **Language**: javascript"));
    assert!(summary.contains("📏 **Code length**: 42 characters"));
    assert!(summary.contains("🏷️ **Node types**: 4 available"));
    assert!(summary.contains("1. `program`"));
    assert!(summary.contains("2. `function_declaration`"));
    assert!(summary.contains("**AST Structure**"));
    assert!(summary.contains("(program (function_declaration"));
}

#[test]
fn test_rule_management_formatting() {
    let summary = ResponseFormatter::format_rule_management_result(
        "Creation",
        true,
        "Rule 'no-console-log' created successfully at /rules/no-console-log.yaml",
    );

    assert!(summary.contains("✅ **Rule Creation Successful**"));
    assert!(summary.contains("Rule 'no-console-log' created successfully"));
}

#[test]
fn test_rule_management_formatting_failed() {
    let summary = ResponseFormatter::format_rule_management_result(
        "deletion",
        false,
        "Rule 'nonexistent' not found",
    );

    assert!(summary.contains("❌ **Rule Deletion Failed**"));
    assert!(summary.contains("Rule 'nonexistent' not found"));
}
