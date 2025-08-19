//! # Type Definitions for ast-grep MCP Service
//!
//! This module contains all the type definitions used throughout the ast-grep MCP service,
//! including parameter and result types for all MCP tools, and various configuration and
//! data structures.
//!
//! ## Key Type Categories
//!
//! - **Search Types**: [`SearchParam`], [`SearchResult`], [`MatchResult`]
//! - **File Search Types**: [`FileSearchParam`], [`FileSearchResult`], [`FileMatchResult`]
//! - **Replace Types**: [`ReplaceParam`], [`ReplaceResult`], [`ChangeResult`]
//! - **File Replace Types**: [`FileReplaceParam`], [`FileReplaceResult`], [`FileDiffResult`]
//! - **Utility Types**: [`MatchStrictness`], [`CursorParam`], [`GenerateAstParam`]
//!
//! ## Important Notes
//!
//! - All search and replace operations use **literal pattern matching**
//! - Commas must be explicitly included in replacement patterns
//! - Struct update syntax (`..Default::default()`) must come last in Rust patterns

use ast_grep_core::NodeMatch;
use ast_grep_core::tree_sitter::StrDoc;
use ast_grep_language::SupportLang as Language;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Controls how strictly patterns match against the syntax tree.
///
/// Different strictness levels allow for more or less precise matching:
/// - Use [`MatchStrictness::Cst`] for exact matching including whitespace
/// - Use [`MatchStrictness::Smart`] for semantic matching (recommended default)
/// - Use [`MatchStrictness::Ast`] for structural matching only
/// - Use [`MatchStrictness::Relaxed`] to ignore comments
/// - Use [`MatchStrictness::Signature`] for signature-only matching
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchStrictness {
    /// Match exact all nodes including whitespace and punctuation
    Cst,
    /// Match all nodes except source trivial nodes (recommended default)
    Smart,
    /// Match only AST nodes, ignoring trivia
    Ast,
    /// Match AST nodes except comments
    Relaxed,
    /// Match AST nodes except comments, without text matching
    Signature,
}

impl From<MatchStrictness> for ast_grep_core::MatchStrictness {
    /// Convert from our MCP service MatchStrictness to ast-grep core MatchStrictness
    fn from(strictness: MatchStrictness) -> Self {
        match strictness {
            MatchStrictness::Cst => ast_grep_core::MatchStrictness::Cst,
            MatchStrictness::Smart => ast_grep_core::MatchStrictness::Smart,
            MatchStrictness::Ast => ast_grep_core::MatchStrictness::Ast,
            MatchStrictness::Relaxed => ast_grep_core::MatchStrictness::Relaxed,
            MatchStrictness::Signature => ast_grep_core::MatchStrictness::Signature,
        }
    }
}

/// Parameters for searching patterns in code strings.
///
/// Used for in-memory pattern matching against a code string. For file-based search,
/// use [`FileSearchParam`] instead.
///
/// # Example
///
/// ```rust
/// use ast_grep_mcp::SearchParam;
///
/// let param = SearchParam {
///     code: "function test() { console.log('hello'); }".to_string(),
///     pattern: "console.log($VAR)".to_string(),
///     language: "javascript".to_string(),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchParam {
    /// The source code to search in
    pub code: String,
    /// The ast-grep pattern to match (e.g., "console.log($VAR)")
    pub pattern: String,
    /// Programming language (e.g., "javascript", "rust", "python")
    pub language: String,
    /// How strictly to match the pattern (default: Smart)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strictness: Option<MatchStrictness>,
    /// CSS-like selector to filter matches to specific sub-nodes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Additional rule context (YAML rule configuration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Number of lines to include before each match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_before: Option<usize>,
    /// Number of lines to include after each match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_after: Option<usize>,
    /// Number of lines to include both before and after each match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_lines: Option<usize>,
}

impl SearchParam {
    /// Create a new SearchParam with the minimum required fields.
    ///
    /// Other fields are set to None and can be configured using struct update syntax.
    pub fn new(code: &str, pattern: &str, language: &str) -> Self {
        Self {
            code: code.to_string(),
            pattern: pattern.to_string(),
            language: language.to_string(),
            ..Default::default()
        }
    }
}

/// Result of a pattern search operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// All matches found for the pattern
    pub matches: Vec<MatchResult>,
    /// Optional summary of matches (used for large result sets)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matches_summary: Option<String>,
}

/// A single match result with position information and captured variables.
///
/// Contains the matched text, its position in the source, and any metavariables
/// captured by the pattern (e.g., `$VAR` in pattern `console.log($VAR)`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    /// The matched text content
    pub text: String,
    /// Starting line number (0-based)
    pub start_line: usize,
    /// Ending line number (0-based)
    pub end_line: usize,
    /// Starting column number (0-based)
    pub start_col: usize,
    /// Ending column number (0-based)
    pub end_col: usize,
    /// Captured metavariables from the pattern (e.g., {"VAR": "'hello'"})
    pub vars: HashMap<String, String>,
    /// Lines of context before the match (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_before: Option<Vec<String>>,
    /// Lines of context after the match (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_after: Option<Vec<String>>,
}

impl MatchResult {
    /// Convert a NodeMatch from ast-grep core into a MatchResult.
    ///
    /// Extracts position information, matched text, and captured metavariables
    /// from the ast-grep NodeMatch structure.
    pub fn from_node_match(node: &NodeMatch<StrDoc<Language>>) -> Self {
        let vars: HashMap<String, String> = node.get_env().clone().into();
        let start_pos = node.get_node().start_pos();
        let end_pos = node.get_node().end_pos();

        MatchResult {
            text: node.text().to_string(),
            start_line: start_pos.line(),
            end_line: end_pos.line(),
            start_col: start_pos.column(node),
            end_col: end_pos.column(node),
            vars,
            context_before: None,
            context_after: None,
        }
    }

    /// Add context lines before and after this match.
    ///
    /// This is used by the context lines feature to provide surrounding code
    /// for better understanding of the match location.
    pub fn with_context(
        mut self,
        context_before: Option<Vec<String>>,
        context_after: Option<Vec<String>>,
    ) -> Self {
        self.context_before = context_before;
        self.context_after = context_after;
        self
    }
}

/// Parameters for searching patterns across multiple files.
///
/// Supports both glob patterns (e.g., `**/*.js`) and direct file paths.
/// Results are paginated using cursor-based pagination for large result sets.
///
/// # Example
///
/// ```rust
/// use ast_grep_mcp::FileSearchParam;
///
/// let param = FileSearchParam {
///     path_pattern: "src/**/*.js".to_string(),
///     pattern: "console.log($VAR)".to_string(),
///     language: "javascript".to_string(),
///     max_results: 10,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchParam {
    /// Glob pattern ("src/**/*.js") or direct file path ("/path/to/file.js")
    pub path_pattern: String,
    /// The ast-grep pattern to match
    pub pattern: String,
    /// Programming language
    pub language: String,
    /// Maximum number of matches to return (default: 20)
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    /// Maximum file size to process in bytes (default: 50MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    /// Pagination cursor for continuing previous search
    pub cursor: Option<CursorParam>,
    /// How strictly to match the pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strictness: Option<MatchStrictness>,
    /// CSS-like selector to filter matches
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Additional rule context (YAML)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Number of lines to include before each match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_before: Option<usize>,
    /// Number of lines to include after each match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_after: Option<usize>,
    /// Number of lines to include both before and after each match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_lines: Option<usize>,
}

impl Default for FileSearchParam {
    fn default() -> Self {
        Self {
            path_pattern: "**/*".to_string(),
            pattern: String::new(),
            language: String::new(),
            max_results: default_max_results(),
            max_file_size: default_max_file_size(),
            cursor: None,
            strictness: None,
            selector: None,
            context: None,
            context_before: None,
            context_after: None,
            context_lines: None,
        }
    }
}

/// Pagination cursor for continuing file-based operations.
///
/// Used internally to track progress through large file sets. The cursor is opaque
/// and base64-encoded for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorParam {
    /// Last file processed in the previous page
    pub last_file_path: String,
    /// Whether the operation has completed (no more results)
    pub is_complete: bool,
}

/// Result of a file-based pattern search operation.
///
/// Contains matches organized by file, with optional pagination cursor for large result sets.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileSearchResult {
    /// Matches organized by file
    pub matches: Vec<FileMatchResult>,
    /// Cursor for fetching next page of results (if any)
    pub next_cursor: Option<CursorResult>,
    /// Total number of files searched
    pub total_files_found: usize,
}

/// Matches found in a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatchResult {
    /// Path to the file containing matches
    pub file_path: String,
    /// File size in bytes
    pub file_size_bytes: u64,
    /// All pattern matches found in this file
    pub matches: Vec<MatchResult>,
    /// SHA-256 hash of the file content for change detection
    pub file_hash: String,
}

/// Pagination cursor returned in API responses.
///
/// Used to continue fetching more results from where the previous request left off.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorResult {
    /// Last file processed in this page
    pub last_file_path: String,
    /// Whether there are more results available
    pub is_complete: bool,
}

/// Parameters for replacing patterns in code strings.
///
/// Used for in-memory pattern replacement. For file-based replacement,
/// use [`FileReplaceParam`] instead.
///
/// # Important Notes
///
/// - **Manual Comma Handling**: You must include commas explicitly in replacement patterns
/// - **Literal Replacement**: ast-grep does exact text replacement, no automatic formatting
/// - **Syntax Responsibility**: Ensure replacement patterns produce valid syntax
///
/// # Example
///
/// ```rust
/// use ast_grep_mcp::ReplaceParam;
///
/// let param = ReplaceParam {
///     code: "var x = 1;".to_string(),
///     pattern: "var $VAR = $VAL;".to_string(),
///     replacement: "let $VAR = $VAL;".to_string(),
///     language: "javascript".to_string(),
///     strictness: None,
///     selector: None,
///     context: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceParam {
    /// The source code to search and replace in
    pub code: String,
    /// The ast-grep pattern to match
    pub pattern: String,
    /// The replacement text (may include metavariables like $VAR)
    pub replacement: String,
    /// Programming language
    pub language: String,
    /// How strictly to match the pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strictness: Option<MatchStrictness>,
    /// CSS-like selector to filter matches
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Additional rule context (YAML)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl ReplaceParam {
    /// Create a new ReplaceParam with the minimum required fields.
    pub fn new(code: &str, pattern: &str, replacement: &str, language: &str) -> Self {
        Self {
            code: code.to_string(),
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            language: language.to_string(),
            strictness: None,
            selector: None,
            context: None,
        }
    }
}

/// Result of a pattern replacement operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReplaceResult {
    /// The code after applying all replacements
    pub new_code: String,
    /// List of all changes made (for diff visualization)
    pub changes: Vec<ChangeResult>,
}

/// A single change made during replacement.
///
/// Represents one pattern match that was replaced, with before/after text
/// and position information for generating diffs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeResult {
    /// Starting line number (0-based)
    pub start_line: usize,
    /// Ending line number (0-based)
    pub end_line: usize,
    /// Starting column number (0-based)
    pub start_col: usize,
    /// Ending column number (0-based)
    pub end_col: usize,
    /// Original text that was replaced
    pub old_text: String,
    /// New text after replacement
    pub new_text: String,
}

/// Parameters for replacing patterns across multiple files.
///
/// The primary tool for bulk code refactoring. Returns token-efficient diffs
/// instead of full file content to minimize API response size.
///
/// # Safety Features
///
/// - **Dry Run Default**: `dry_run` defaults to `true` for safety
/// - **File Size Limits**: Large files are skipped to prevent timeouts
/// - **Result Limits**: Pagination prevents overwhelming responses
///
/// # Important Notes
///
/// - **Manual Comma Handling**: Include commas explicitly in replacement patterns
/// - **Syntax Responsibility**: Ensure replacements produce valid syntax
/// - **Test First**: Always preview with `dry_run: true` before applying changes
///
/// # Example
///
/// ```rust
/// use ast_grep_mcp::FileReplaceParam;
///
/// let param = FileReplaceParam {
///     path_pattern: "src/**/*.js".to_string(),
///     pattern: "var $VAR = $VAL".to_string(),
///     replacement: "const $VAR = $VAL".to_string(),
///     language: "javascript".to_string(),
///     dry_run: true, // Preview first!
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReplaceParam {
    /// Glob pattern or direct file path to search
    pub path_pattern: String,
    /// The ast-grep pattern to match
    pub pattern: String,
    /// The replacement text (may include metavariables)
    pub replacement: String,
    /// Programming language
    pub language: String,
    /// Maximum number of changes to process (default: 10000)
    #[serde(default = "default_max_results_large")]
    pub max_results: usize,
    /// Maximum file size to process in bytes (default: 50MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    /// If true, preview changes without modifying files (default: true)
    #[serde(default = "default_true")]
    pub dry_run: bool,
    /// If true, return only summary statistics (default: false)
    #[serde(default = "default_false")]
    pub summary_only: bool,
    /// If true, include sample changes in summary mode (default: false)
    #[serde(default = "default_false")]
    pub include_samples: bool,
    /// Maximum number of sample changes per file (default: 3)
    #[serde(default = "default_max_samples")]
    pub max_samples: usize,
    /// Pagination cursor for continuing previous operation
    pub cursor: Option<CursorParam>,
    /// How strictly to match the pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strictness: Option<MatchStrictness>,
    /// CSS-like selector to filter matches
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Additional rule context (YAML)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl Default for FileReplaceParam {
    fn default() -> Self {
        Self {
            path_pattern: "**/*".to_string(),
            pattern: String::new(),
            replacement: String::new(),
            language: String::new(),
            max_results: default_max_results_large(),
            max_file_size: default_max_file_size(),
            dry_run: default_true(),
            summary_only: default_false(),
            include_samples: default_false(),
            max_samples: default_max_samples(),
            cursor: None,
            strictness: None,
            selector: None,
            context: None,
        }
    }
}

/// Result of a file-based replacement operation.
///
/// Contains either detailed diffs or summary statistics, depending on the request parameters.
/// Uses token-efficient representation to minimize API response size.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileReplaceResult {
    /// Detailed file-by-file diff results (when summary_only=false)
    pub file_results: Vec<FileDiffResult>,
    /// Summary statistics per file (when summary_only=true)
    pub summary_results: Vec<FileSummaryResult>,
    /// Cursor for fetching next page of results
    pub next_cursor: Option<CursorResult>,
    /// Total number of files processed
    pub total_files_found: usize,
    /// Whether this was a dry run (no actual changes made)
    pub dry_run: bool,
    /// Total number of individual changes across all files
    pub total_changes: usize,
    /// Number of files that had at least one change
    pub files_with_changes: usize,
}

/// Detailed diff information for a single file.
///
/// Contains line-by-line changes for token-efficient diff visualization.
/// Used when `summary_only=false` in FileReplaceParam.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileDiffResult {
    /// Path to the modified file
    pub file_path: String,
    /// File size in bytes
    pub file_size_bytes: u64,
    /// List of all changes made to this file
    pub changes: Vec<ChangeResult>,
    /// Total number of changes in this file
    pub total_changes: usize,
    /// SHA-256 hash of the original file content
    pub file_hash: String,
}

/// A single line change within a file diff.
///
/// Represents a line-level change for diff visualization.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileDiffChange {
    /// Line number where the change occurred (1-based)
    pub line_number: usize,
    /// Original line content
    pub old_content: String,
    /// New line content after replacement
    pub new_content: String,
}

/// Summary statistics for a single file's changes.
///
/// Used when `summary_only=true` to provide change counts without full diff details.
/// Minimizes token usage for large refactoring operations.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileSummaryResult {
    /// Path to the modified file
    pub file_path: String,
    /// File size in bytes
    pub file_size_bytes: u64,
    /// Total number of pattern matches replaced
    pub total_changes: usize,
    /// Number of lines affected by changes
    pub lines_changed: usize,
    /// SHA-256 hash of the original file content
    pub file_hash: String,
    /// Sample changes for preview (limited by max_samples)
    pub sample_changes: Vec<ChangeResult>,
}

/// Parameters for listing supported programming languages.
///
/// This is an empty struct as no parameters are needed to list supported languages.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListLanguagesParam {}

/// Result containing all supported programming languages.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListLanguagesResult {
    /// List of language names (e.g., ["javascript", "rust", "python"])
    pub languages: Vec<String>,
}

/// Parameters for generating syntax tree representations.
///
/// Essential for LLM users to understand Tree-sitter node structure and discover
/// available node kinds for writing Kind rules.
///
/// # Example
///
/// ```rust
/// use ast_grep_mcp::GenerateAstParam;
///
/// let param = GenerateAstParam {
///     code: "function test() { return 42; }".to_string(),
///     language: "javascript".to_string(),
/// };
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateAstParam {
    /// Source code to parse into AST
    pub code: String,
    /// Programming language for parsing
    pub language: String,
}

/// Result containing AST representation and discovered node kinds.
///
/// The `node_kinds` field is particularly useful for writing Kind rules,
/// as it shows all Tree-sitter node types present in the parsed code.
#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateAstResult {
    /// String representation of the syntax tree
    pub ast: String,
    /// The programming language that was parsed
    pub language: String,
    /// Length of the input code in characters
    pub code_length: usize,
    /// All Tree-sitter node kinds found (e.g., ["function_declaration", "identifier"])
    pub node_kinds: Vec<String>,
}

/// Parameters for analyzing code fragments for refactoring potential.
///
/// This tool provides comprehensive analysis for extract-function refactoring,
/// including variable dependency analysis, return value inference, and side effect detection.
///
/// # Usage
///
/// ```json
/// {
///     "fragment": "let sum = x + y; console.log(sum);",
///     "context": "function calculate() { let x = 5; let y = 10; let sum = x + y; console.log(sum); return sum; }",
///     "language": "javascript"
/// }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeRefactoringParam {
    /// The code fragment to analyze for extraction
    pub fragment: String,
    /// The full context code containing the fragment
    pub context: String,
    /// Programming language for parsing
    pub language: String,
}

/// Result of refactoring analysis containing comprehensive extraction insights.
///
/// Provides all information needed to perform safe extract-function refactoring,
/// including parameter requirements, return strategy, and potential side effects.
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeRefactoringResult {
    /// Variables that need to be passed as parameters
    pub external_reads: Vec<VariableUsageInfo>,
    /// Variables modified by the fragment
    pub external_writes: Vec<VariableUsageInfo>,
    /// Variables declared within the fragment
    pub internal_declarations: Vec<VariableUsageInfo>,
    /// Detected return values
    pub return_values: Vec<ReturnValueInfo>,
    /// Recommended return strategy
    pub suggested_return_strategy: Option<ReturnStrategyInfo>,
    /// Side effects detected in the fragment
    pub side_effects: Vec<SideEffectInfo>,
    /// Suggested function signature
    pub suggested_signature: FunctionSignatureInfo,
    /// Scope analysis information
    pub scope_info: ScopeAnalysisInfo,
}

/// Parameters for the integrated extract_function tool
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractFunctionParam {
    /// Code fragment to extract into a function
    pub fragment: String,
    /// Full context code containing the fragment
    pub context: String,
    /// Programming language
    pub language: String,
    /// Name for the extracted function
    pub function_name: String,
    /// Whether to preview changes only (default: true)
    pub dry_run: Option<bool>,
    /// Path pattern for file operations (when working with files)
    pub path_pattern: Option<String>,
}

/// Result of the integrated extract_function operation
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractFunctionResult {
    /// Analysis results from the fragment
    pub analysis: AnalyzeRefactoringResult,
    /// The generated extracted function code
    pub extracted_function: String,
    /// The modified original code with function call
    pub modified_context: String,
    /// Whether this was a dry run
    pub dry_run: bool,
    /// Success/failure status
    pub success: bool,
    /// Any warnings or notes
    pub messages: Vec<String>,
}

/// Information about variable usage in the analyzed fragment
#[derive(Debug, Serialize, Deserialize)]
pub struct VariableUsageInfo {
    /// Variable name
    pub name: String,
    /// Inferred or explicit type (if available)
    pub var_type: Option<String>,
    /// How the variable is used (read, write, declaration)
    pub usage_type: String,
    /// Scope level where the variable is accessible
    pub scope_level: usize,
    /// Line number of first usage
    pub first_usage_line: usize,
}

/// Information about return values detected in the fragment
#[derive(Debug, Serialize, Deserialize)]
pub struct ReturnValueInfo {
    /// The return expression
    pub expression: String,
    /// Inferred type of the return value
    pub inferred_type: Option<String>,
    /// Whether this return is the result of a mutation
    pub is_mutation_result: bool,
}

/// Recommended return strategy for the extracted function
#[derive(Debug, Serialize, Deserialize)]
pub struct ReturnStrategyInfo {
    /// Type of return strategy
    pub strategy_type: String, // "single", "multiple", "in_place", "void"
    /// Description of the strategy
    pub description: String,
    /// Specific return expression (for single returns)
    pub expression: Option<String>,
    /// Multiple return values (for multiple returns)
    pub values: Option<Vec<String>>,
    /// Parameters that will be modified in place
    pub modified_params: Option<Vec<String>>,
    /// Inferred return type
    pub return_type: Option<String>,
}

/// Information about side effects detected in the fragment
#[derive(Debug, Serialize, Deserialize)]
pub struct SideEffectInfo {
    /// Type of side effect
    pub effect_type: String, // "function_call", "global_mutation", "io_operation", "async_operation", "dom_manipulation", "network_operation"
    /// Description of the side effect
    pub description: String,
    /// Target of the side effect (function name, variable, etc.)
    pub target: Option<String>,
    /// Additional details specific to the side effect type
    pub details: HashMap<String, String>,
}

/// Suggested function signature for the extracted code
#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionSignatureInfo {
    /// Suggested function name
    pub name: String,
    /// Parameter list
    pub parameters: Vec<String>,
    /// Return type information
    pub return_info: String,
    /// Complete function signature
    pub full_signature: String,
    /// Whether the function is pure (no side effects)
    pub is_pure: bool,
}

/// Scope analysis information for the fragment
#[derive(Debug, Serialize, Deserialize)]
pub struct ScopeAnalysisInfo {
    /// Current scope type
    pub current_scope_type: String,
    /// Scope depth
    pub scope_depth: usize,
    /// Whether the fragment crosses scope boundaries
    pub crosses_boundaries: bool,
    /// Any scope violations detected
    pub violations: Vec<String>,
    /// Instance members accessed (for class contexts)
    pub instance_members: Vec<String>,
}

// Default functions for serde deserialization

/// Default maximum results for search operations (20)
pub fn default_max_results() -> usize {
    20
}

/// Default maximum results for large operations like file replacement (10,000)
pub fn default_max_results_large() -> usize {
    10000
}

/// Default maximum file size to process (50 MB)
pub fn default_max_file_size() -> u64 {
    50 * 1024 * 1024
}

/// Default value for boolean fields that should be true
pub fn default_true() -> bool {
    true
}

/// Default value for boolean fields that should be false
pub fn default_false() -> bool {
    false
}

/// Default maximum number of sample changes to include (3)
pub fn default_max_samples() -> usize {
    3
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast_grep_core::{AstGrep, Pattern};
    use ast_grep_language::SupportLang as Language;

    #[test]
    fn test_match_result_from_node_match() {
        let code = "console.log('test');";
        let lang = Language::JavaScript;
        let ast = AstGrep::new(code, lang);
        let pattern = Pattern::new("console.log($VAR)", lang);

        if let Some(node_match) = ast.root().find(pattern) {
            let match_result = MatchResult::from_node_match(&node_match);

            assert!(match_result.text.contains("console.log"));
            assert_eq!(match_result.start_line, 0); // ast-grep uses 0-based line indexing
            assert_eq!(match_result.start_col, 0);
            // Variables should contain VAR
            assert!(match_result.vars.contains_key("VAR"));
        } else {
            panic!("Pattern should match");
        }
    }

    #[test]
    fn test_file_search_param_default() {
        let param = FileSearchParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "console.log($VAR)".to_string(),
            language: "javascript".to_string(),
            ..Default::default()
        };

        assert_eq!(param.max_results, 20);
        assert_eq!(param.max_file_size, 50 * 1024 * 1024);
        assert!(param.cursor.is_none());
    }

    #[test]
    fn test_file_replace_param_default() {
        let param = FileReplaceParam {
            path_pattern: "**/*.js".to_string(),
            pattern: "var $VAR = $VALUE;".to_string(),
            replacement: "let $VAR = $VALUE;".to_string(),
            language: "javascript".to_string(),
            ..Default::default()
        };

        assert_eq!(param.max_results, 10000);
        assert_eq!(param.max_file_size, 50 * 1024 * 1024);
        assert!(param.dry_run); // Should default to true
        assert!(!param.summary_only); // Should default to false
        assert!(!param.include_samples); // Should default to false
        assert_eq!(param.max_samples, 3);
        assert!(param.cursor.is_none());
    }

    #[test]
    fn test_search_param_serialization() {
        let param = SearchParam::new("console.log('test');", "console.log($VAR)", "javascript");

        let serialized = serde_json::to_string(&param).unwrap();
        let deserialized: SearchParam = serde_json::from_str(&serialized).unwrap();

        assert_eq!(param.code, deserialized.code);
        assert_eq!(param.pattern, deserialized.pattern);
        assert_eq!(param.language, deserialized.language);
    }

    #[test]
    fn test_replace_param_serialization() {
        let param = ReplaceParam::new(
            "var x = 1;",
            "var $VAR = $VALUE;",
            "let $VAR = $VALUE;",
            "javascript",
        );

        let serialized = serde_json::to_string(&param).unwrap();
        let deserialized: ReplaceParam = serde_json::from_str(&serialized).unwrap();

        assert_eq!(param.code, deserialized.code);
        assert_eq!(param.pattern, deserialized.pattern);
        assert_eq!(param.replacement, deserialized.replacement);
        assert_eq!(param.language, deserialized.language);
    }

    #[test]
    fn test_cursor_param_serialization() {
        let cursor = CursorParam {
            last_file_path: "test/file.js".to_string(),
            is_complete: false,
        };

        let serialized = serde_json::to_string(&cursor).unwrap();
        let deserialized: CursorParam = serde_json::from_str(&serialized).unwrap();

        assert_eq!(cursor.last_file_path, deserialized.last_file_path);
        assert_eq!(cursor.is_complete, deserialized.is_complete);
    }

    #[test]
    fn test_change_result_creation() {
        let change = ChangeResult {
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 10,
            old_text: "var x = 1;".to_string(),
            new_text: "let x = 1;".to_string(),
        };

        assert_eq!(change.start_line, 1);
        assert_eq!(change.old_text, "var x = 1;");
        assert_eq!(change.new_text, "let x = 1;");
    }

    #[test]
    fn test_file_diff_result_creation() {
        let changes = vec![
            ChangeResult {
                start_line: 1,
                end_line: 1,
                start_col: 0,
                end_col: 10,
                old_text: "var x = 1;".to_string(),
                new_text: "let x = 1;".to_string(),
            },
            ChangeResult {
                start_line: 2,
                end_line: 2,
                start_col: 0,
                end_col: 10,
                old_text: "var y = 2;".to_string(),
                new_text: "let y = 2;".to_string(),
            },
        ];

        let diff_result = FileDiffResult {
            file_path: "test.js".to_string(),
            file_size_bytes: 1024,
            changes: changes.clone(),
            total_changes: changes.len(),
            file_hash: "abc123".to_string(),
        };

        assert_eq!(diff_result.file_path, "test.js");
        assert_eq!(diff_result.total_changes, 2);
        assert_eq!(diff_result.changes.len(), 2);
    }

    #[test]
    fn test_file_summary_result_creation() {
        let sample_changes = vec![ChangeResult {
            start_line: 1,
            end_line: 1,
            start_col: 0,
            end_col: 10,
            old_text: "var x = 1;".to_string(),
            new_text: "let x = 1;".to_string(),
        }];

        let summary_result = FileSummaryResult {
            file_path: "test.js".to_string(),
            file_size_bytes: 1024,
            total_changes: 5,
            lines_changed: 3,
            file_hash: "abc123".to_string(),
            sample_changes,
        };

        assert_eq!(summary_result.file_path, "test.js");
        assert_eq!(summary_result.total_changes, 5);
        assert_eq!(summary_result.lines_changed, 3);
        assert_eq!(summary_result.sample_changes.len(), 1);
    }

    #[test]
    fn test_generate_ast_param_serialization() {
        let param = GenerateAstParam {
            code: "function test() {}".to_string(),
            language: "javascript".to_string(),
        };

        let serialized = serde_json::to_string(&param).unwrap();
        let deserialized: GenerateAstParam = serde_json::from_str(&serialized).unwrap();

        assert_eq!(param.code, deserialized.code);
        assert_eq!(param.language, deserialized.language);
    }

    #[test]
    fn test_default_functions() {
        assert_eq!(default_max_results(), 20);
        assert_eq!(default_max_results_large(), 10000);
        assert_eq!(default_max_file_size(), 50 * 1024 * 1024);
        assert!(default_true());
        assert!(!default_false());
        assert_eq!(default_max_samples(), 3);
    }
}
