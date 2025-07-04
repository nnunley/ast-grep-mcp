use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Basic search and replace types
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchParam {
    pub code: String,
    pub pattern: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub matches: Vec<MatchResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResult {
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchParam {
    pub path_pattern: String,
    pub pattern: String,
    pub language: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    pub cursor: Option<CursorParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorParam {
    pub last_file_path: String,
    pub is_complete: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSearchResult {
    pub matches: Vec<FileMatchResult>,
    pub next_cursor: Option<CursorResult>,
    pub total_files_found: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMatchResult {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub matches: Vec<MatchResult>,
    pub file_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CursorResult {
    pub last_file_path: String,
    pub is_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceParam {
    pub code: String,
    pub pattern: String,
    pub replacement: String,
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplaceResult {
    pub new_code: String,
    pub changes: Vec<ChangeResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeResult {
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReplaceParam {
    pub path_pattern: String,
    pub pattern: String,
    pub replacement: String,
    pub language: String,
    #[serde(default = "default_max_results_large")]
    pub max_results: usize,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default = "default_false")]
    pub summary_only: bool,
    #[serde(default = "default_false")]
    pub include_samples: bool,
    #[serde(default = "default_max_samples")]
    pub max_samples: usize,
    pub cursor: Option<CursorParam>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileReplaceResult {
    pub file_results: Vec<FileDiffResult>,
    pub summary_results: Vec<FileSummaryResult>,
    pub next_cursor: Option<CursorResult>,
    pub total_files_found: usize,
    pub dry_run: bool,
    pub total_changes: usize,
    pub files_with_changes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileDiffResult {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub changes: Vec<ChangeResult>,
    pub total_changes: usize,
    pub file_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSummaryResult {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub total_changes: usize,
    pub lines_changed: usize,
    pub file_hash: String,
    pub sample_changes: Vec<ChangeResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListLanguagesParam {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListLanguagesResult {
    pub languages: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationParam {}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentationResult {
    pub content: String,
}

// Catalog types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCatalogRulesParam {
    pub language: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCatalogRulesResult {
    pub rules: Vec<CatalogRuleInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub language: String,
    pub category: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCatalogRuleParam {
    pub rule_url: String,
    pub rule_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCatalogRuleResult {
    pub rule_id: String,
    pub imported: bool,
    pub message: String,
}

// Default functions for serde
fn default_max_results() -> usize { 100 }
fn default_max_results_large() -> usize { 10000 }
fn default_max_file_size() -> u64 { 50 * 1024 * 1024 }
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_max_samples() -> usize { 3 }