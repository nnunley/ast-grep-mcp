use std::path::PathBuf;

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
