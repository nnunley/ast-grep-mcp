//! # Service Configuration
//!
//! Configuration structures and defaults for the ast-grep MCP service.
//! These settings control performance, resource limits, and file system access.

use crate::sg_config::SgConfig;
use std::path::{Path, PathBuf};

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
    /// Additional rule directories from sgconfig.yml
    pub additional_rule_dirs: Vec<PathBuf>,
    /// Utility rule directories from sgconfig.yml
    pub util_dirs: Vec<PathBuf>,
    /// Path to the loaded sgconfig.yml (if any)
    pub sg_config_path: Option<PathBuf>,
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
            additional_rule_dirs: Vec::new(),
            util_dirs: Vec::new(),
            sg_config_path: None,
        }
    }
}

impl ServiceConfig {
    /// Load configuration with sgconfig.yml discovery
    pub fn with_sg_config(self, config_path: Option<&Path>) -> Self {
        let start_dir = self
            .root_directories
            .first()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        // Try to load sgconfig.yml
        let sg_config_result = if let Some(path) = config_path {
            // Use specified path
            SgConfig::from_file(path).map(|config| (path.to_path_buf(), config))
        } else {
            // Discover sgconfig.yml
            match SgConfig::discover(&start_dir) {
                Ok(Some((path, config))) => Ok((path, config)),
                Ok(None) => return self, // No config found, return unchanged
                Err(e) => {
                    eprintln!("Warning: Failed to load sgconfig.yml: {e}");
                    return self;
                }
            }
        };

        match sg_config_result {
            Ok((path, mut sg_config)) => {
                // Resolve relative paths in sg_config
                if let Some(parent) = path.parent() {
                    sg_config.resolve_paths(parent);
                }

                // Merge configurations
                self.merge_sg_config(sg_config, Some(path))
            }
            Err(e) => {
                eprintln!("Warning: Failed to load sgconfig.yml: {e}");
                self
            }
        }
    }

    /// Merge SgConfig into ServiceConfig
    fn merge_sg_config(mut self, sg_config: SgConfig, config_path: Option<PathBuf>) -> Self {
        self.additional_rule_dirs = sg_config.rule_dirs;
        self.util_dirs = sg_config.util_dirs;
        self.sg_config_path = config_path;

        // TODO: Handle test_configs and custom_languages when needed

        self
    }

    /// Get all rule directories (including the main rules_directory and additional ones)
    pub fn all_rule_directories(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.rules_directory.clone()];
        dirs.extend(self.additional_rule_dirs.clone());
        dirs
    }
}
