//! # Service Configuration
//!
//! Configuration structures and defaults for the ast-grep MCP service.
//! These settings control performance, resource limits, and file system access.

use std::path::PathBuf;

/// Configuration for the ast-grep MCP service.
///
/// Controls various aspects of service behavior including resource limits,
/// file system access, and performance tuning.
///
/// # Example
///
/// ```rust
/// use ast_grep_mcp::config::ServiceConfig;
/// use std::path::PathBuf;
///
/// let config = ServiceConfig {
///     max_file_size: 10 * 1024 * 1024, // 10MB limit
///     root_directories: vec![PathBuf::from("src")],
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Maximum file size to process (in bytes)
    pub max_file_size: u64,
    /// Maximum number of concurrent file operations
    pub max_concurrency: usize,
    /// Maximum number of results to return per search
    pub limit: usize,
    /// Root directories for file search (defaults to current working directory)
    pub root_directories: Vec<PathBuf>,
    /// Directory for storing custom rules created by LLMs
    pub rules_directory: PathBuf,
    /// Maximum number of compiled patterns to cache (default: 1000)
    pub pattern_cache_size: usize,
}

impl Default for ServiceConfig {
    /// Create a ServiceConfig with sensible defaults.
    ///
    /// Default values:
    /// - `max_file_size`: 50MB
    /// - `max_concurrency`: 10 concurrent operations
    /// - `limit`: 100 results per search
    /// - `root_directories`: Current working directory
    /// - `rules_directory`: `.ast-grep-rules` in current directory
    /// - `pattern_cache_size`: 1000 cached compiled patterns
    fn default() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024, // 50MB
            max_concurrency: 10,
            limit: 100,
            root_directories: vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))],
            rules_directory: PathBuf::from(".ast-grep-rules"),
            pattern_cache_size: 1000, // Cache up to 1000 compiled patterns
        }
    }
}
