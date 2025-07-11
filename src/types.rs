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
//! - **Pattern Suggestion Types**: [`SuggestPatternsParam`], [`SuggestPatternsResult`]
//! - **Debug Types**: [`DebugPatternParam`], [`DebugAstParam`], [`DebugFormat`]
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

/// Parameters for pattern suggestion functionality.
///
/// This allows users to provide code examples and get suggested ast-grep patterns
/// that would match those examples, reducing the complexity of writing patterns manually.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestPatternsParam {
    /// Code examples to analyze for pattern generation
    pub code_examples: Vec<String>,
    /// Programming language for the code examples
    pub language: String,
    /// Maximum number of pattern suggestions to return (default: 5)
    pub max_suggestions: Option<usize>,
    /// Specificity levels to include: "exact", "specific", "general"
    pub specificity_levels: Option<Vec<String>>,
}

/// Result containing suggested patterns for the given code examples.
#[derive(Debug, Serialize, Deserialize)]
pub struct SuggestPatternsResult {
    /// List of pattern suggestions with confidence scores
    pub suggestions: Vec<PatternSuggestion>,
    /// The programming language analyzed
    pub language: String,
    /// Total number of suggestions generated
    pub total_suggestions: usize,
}

/// A single pattern suggestion with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSuggestion {
    /// The suggested ast-grep pattern
    pub pattern: String,
    /// Confidence score from 0.0 to 1.0
    pub confidence: f64,
    /// How specific vs general this pattern is
    pub specificity: SpecificityLevel,
    /// Human-readable explanation of what the pattern matches
    pub explanation: String,
    /// Indices of input examples this pattern matches
    pub matching_examples: Vec<usize>,
    /// Tree-sitter node kinds involved in this pattern
    pub node_kinds: Vec<String>,
}

/// Specificity level of a suggested pattern.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpecificityLevel {
    /// Matches the exact structure with minimal metavariables
    Exact,
    /// Matches the core structure with some flexibility
    Specific,
    /// Matches broadly with many metavariables
    General,
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
///     ..Default::default()
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

/// Parameters for retrieving documentation.
///
/// This is an empty struct as no parameters are needed for documentation.
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationParam {}

/// Result containing comprehensive usage documentation.
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationResult {
    /// Markdown-formatted documentation content
    pub content: String,
}

/// Parameters for listing rules from the ast-grep catalog.
///
/// The catalog contains community-contributed rules for common patterns and refactoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCatalogRulesParam {
    /// Filter rules by programming language (optional)
    pub language: Option<String>,
    /// Filter rules by category (optional, e.g., "best-practices")
    pub category: Option<String>,
}

/// Result containing available catalog rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCatalogRulesResult {
    /// List of available rules with metadata
    pub rules: Vec<CatalogRuleInfo>,
}

/// Information about a rule in the ast-grep catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRuleInfo {
    /// Unique identifier for the rule
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what the rule does
    pub description: String,
    /// Programming language this rule applies to
    pub language: String,
    /// Category (e.g., "best-practices", "security")
    pub category: String,
    /// URL to the rule's source or documentation
    pub url: String,
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

/// Debug format type for pattern and AST visualization.
///
/// Different debug formats provide different levels of detail:
/// - `Pattern`: Shows how the pattern is parsed and interpreted
/// - `Ast`: Shows the Abstract Syntax Tree (semantic structure)
/// - `Cst`: Shows the Concrete Syntax Tree (all tokens and trivia)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DebugFormat {
    /// Show pattern parsing and matching structure
    Pattern,
    /// Show Abstract Syntax Tree (semantic nodes only)
    Ast,
    /// Show Concrete Syntax Tree (all tokens including whitespace)
    Cst,
}

/// Parameters for pattern debugging functionality.
///
/// This helps users understand how their patterns are parsed and what they match against.
///
/// # Example
///
/// ```rust
/// use ast_grep_mcp::{DebugPatternParam, DebugFormat};
///
/// let param = DebugPatternParam {
///     pattern: "$FN($ARG)".to_string(),
///     language: "javascript".to_string(),
///     sample_code: Some("console.log('test')".to_string()),
///     format: DebugFormat::Pattern,
/// };
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct DebugPatternParam {
    /// The ast-grep pattern to debug
    pub pattern: String,
    /// Programming language for the pattern
    pub language: String,
    /// Optional sample code to test the pattern against
    pub sample_code: Option<String>,
    /// Debug format type
    pub format: DebugFormat,
}

/// Result containing pattern debug information.
#[derive(Debug, Serialize, Deserialize)]
pub struct DebugPatternResult {
    /// The original pattern that was debugged
    pub pattern: String,
    /// Programming language
    pub language: String,
    /// Debug format used
    pub format: DebugFormat,
    /// Detailed debug information
    pub debug_info: String,
    /// If sample code was provided, show matches
    pub sample_matches: Option<Vec<MatchResult>>,
    /// Explanation of what the pattern matches
    pub explanation: String,
}

/// Enhanced parameters for AST generation with debug options.
///
/// Extends the basic GenerateAstParam with additional debug formatting options.
///
/// # Example
///
/// ```rust
/// use ast_grep_mcp::{DebugAstParam, DebugFormat};
///
/// let param = DebugAstParam {
///     code: "function test() { return 42; }".to_string(),
///     language: "javascript".to_string(),
///     format: DebugFormat::Cst,
///     include_trivia: true,
/// };
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct DebugAstParam {
    /// Source code to parse into AST
    pub code: String,
    /// Programming language for parsing
    pub language: String,
    /// Debug format type (AST or CST)
    pub format: DebugFormat,
    /// Include trivia (whitespace, comments) in CST format
    #[serde(default = "default_true")]
    pub include_trivia: bool,
}

/// Enhanced result containing detailed AST/CST information.
#[derive(Debug, Serialize, Deserialize)]
pub struct DebugAstResult {
    /// String representation of the syntax tree
    pub tree: String,
    /// The programming language that was parsed
    pub language: String,
    /// Debug format used
    pub format: DebugFormat,
    /// Length of the input code in characters
    pub code_length: usize,
    /// All Tree-sitter node kinds found
    pub node_kinds: Vec<String>,
    /// Tree statistics
    pub tree_stats: TreeStats,
}

/// Statistics about the parsed syntax tree.
#[derive(Debug, Serialize, Deserialize)]
pub struct TreeStats {
    /// Total number of nodes in the tree
    pub total_nodes: usize,
    /// Number of leaf nodes (terminals)
    pub leaf_nodes: usize,
    /// Maximum depth of the tree
    pub max_depth: usize,
    /// Number of error nodes
    pub error_nodes: usize,
}

/// Parameters for importing a rule from the ast-grep catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCatalogRuleParam {
    /// URL of the catalog rule to import
    pub rule_url: String,
    /// Optional custom ID for the imported rule (defaults to URL-based ID)
    pub rule_id: Option<String>,
}

/// Result of importing a catalog rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCatalogRuleResult {
    /// ID of the imported rule (for future reference)
    pub rule_id: String,
    /// Whether the rule was successfully imported
    pub imported: bool,
    /// Human-readable status message
    pub message: String,
}

/// Configuration for extracting embedded languages from host languages.
///
/// This defines how to extract code blocks from one language that contain
/// code written in another language (e.g., JavaScript in HTML, SQL in Python).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedLanguageConfig {
    /// The host language (e.g., "html", "python")
    pub host_language: String,
    /// The embedded language (e.g., "javascript", "sql")
    pub embedded_language: String,
    /// Pattern to match the embedded code in the host language
    pub extraction_pattern: String,
    /// Optional selector to narrow down the extraction
    pub selector: Option<String>,
    /// Optional context pattern for more precise matching
    pub context: Option<String>,
}

/// Parameters for searching in embedded languages.
#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddedSearchParam {
    /// Code containing embedded languages
    pub code: String,
    /// Pattern to search for in the embedded language
    pub pattern: String,
    /// Configuration for extracting embedded code
    pub embedded_config: EmbeddedLanguageConfig,
    /// Optional match strictness
    pub strictness: Option<MatchStrictness>,
}

/// Result of searching in embedded languages.
#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddedSearchResult {
    /// Matches found in embedded code blocks
    pub matches: Vec<EmbeddedMatchResult>,
    /// Host language used for extraction
    pub host_language: String,
    /// Embedded language searched
    pub embedded_language: String,
    /// Total number of embedded code blocks found
    pub total_embedded_blocks: usize,
}

/// A match result from embedded language search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedMatchResult {
    /// The matched text
    pub text: String,
    /// Starting line number in host file
    pub start_line: usize,
    /// Ending line number in host file
    pub end_line: usize,
    /// Starting column number in host file
    pub start_col: usize,
    /// Ending column number in host file
    pub end_col: usize,
    /// Host context description
    pub host_context: String,
    /// Index of the embedded code block (0-based)
    pub embedded_block_index: usize,
    /// Captured metavariables from the pattern
    pub vars: HashMap<String, String>,
}

/// Parameters for file-based embedded language search.
#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddedFileSearchParam {
    /// Path pattern (glob)
    pub path_pattern: String,
    /// Pattern to search for in embedded language
    pub pattern: String,
    /// Embedded language configuration
    pub embedded_config: EmbeddedLanguageConfig,
    /// Maximum number of results to return
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    /// Maximum file size to process
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    /// Pagination cursor
    pub cursor: Option<CursorParam>,
    /// Optional match strictness
    pub strictness: Option<MatchStrictness>,
}

/// Result of file-based embedded language search.
#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddedFileSearchResult {
    /// Files containing matches
    pub matches: Vec<EmbeddedFileMatchResult>,
    /// Pagination cursor for next page
    pub next_cursor: Option<CursorResult>,
    /// Total number of files found
    pub total_files_found: usize,
}

/// Match result for a file containing embedded languages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedFileMatchResult {
    /// File path
    pub file_path: String,
    /// File size in bytes
    pub file_size_bytes: u64,
    /// Matches found in embedded code blocks
    pub matches: Vec<EmbeddedMatchResult>,
    /// Hash of the file content
    pub file_hash: String,
    /// Total embedded code blocks found in file
    pub total_blocks: usize,
}

// Default functions for serde deserialization

/// Default maximum results for search operations (20)
fn default_max_results() -> usize {
    20
}

/// Default maximum results for large operations like file replacement (10,000)
fn default_max_results_large() -> usize {
    10000
}

/// Default maximum file size to process (50 MB)
fn default_max_file_size() -> u64 {
    50 * 1024 * 1024
}

/// Default value for boolean fields that should be true
fn default_true() -> bool {
    true
}

/// Default value for boolean fields that should be false
fn default_false() -> bool {
    false
}

/// Default maximum number of sample changes to include (3)
fn default_max_samples() -> usize {
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
    fn test_list_catalog_rules_param_optional_fields() {
        let param1 = ListCatalogRulesParam {
            language: Some("javascript".to_string()),
            category: None,
        };

        let param2 = ListCatalogRulesParam {
            language: None,
            category: Some("best-practices".to_string()),
        };

        assert!(param1.language.is_some());
        assert!(param1.category.is_none());
        assert!(param2.language.is_none());
        assert!(param2.category.is_some());
    }

    #[test]
    fn test_import_catalog_rule_result() {
        let result = ImportCatalogRuleResult {
            rule_id: "test-rule".to_string(),
            imported: true,
            message: "Successfully imported".to_string(),
        };

        assert_eq!(result.rule_id, "test-rule");
        assert!(result.imported);
        assert!(result.message.contains("Successfully"));
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
