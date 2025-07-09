//! # Project Configuration (sgconfig.yml)
//!
//! Support for reading and parsing ast-grep's sgconfig.yml configuration files.
//! This allows the MCP service to integrate with existing ast-grep projects.

use crate::errors::ServiceError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration structure for sgconfig.yml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SgConfig {
    /// Directories where ast-grep YAML rules are stored
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_dirs: Vec<PathBuf>,

    /// Test configurations for rule testing
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_configs: Vec<TestConfig>,

    /// Directories containing utility rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub util_dirs: Vec<PathBuf>,

    /// Custom language configurations
    #[serde(default, skip_serializing_if = "CustomLanguages::is_empty")]
    pub custom_languages: CustomLanguages,
}

/// Test configuration for rule testing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConfig {
    /// Directory containing test cases
    pub test_dir: PathBuf,

    /// Directory for storing test snapshots (relative to test_dir)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_dir: Option<PathBuf>,
}

/// Custom language configurations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomLanguages {
    #[serde(flatten)]
    pub languages: std::collections::HashMap<String, CustomLanguage>,
}

impl CustomLanguages {
    pub fn is_empty(&self) -> bool {
        self.languages.is_empty()
    }
}

/// Configuration for a custom language
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomLanguage {
    /// Path to the dynamic library for this language
    pub library_path: PathBuf,

    /// File extensions for this language
    pub extensions: Vec<String>,

    /// Character to use instead of $ for metavariables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expando_char: Option<char>,
}

impl SgConfig {
    /// Load configuration from a sgconfig.yml file
    pub fn from_file(path: &Path) -> Result<Self, ServiceError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ServiceError::Internal(format!("Failed to read sgconfig.yml: {e}")))?;

        Self::from_yaml(&content)
    }

    /// Parse configuration from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self, ServiceError> {
        serde_yaml::from_str(yaml).map_err(ServiceError::from)
    }

    /// Find sgconfig.yml by traversing up the directory tree
    pub fn discover(start_dir: &Path) -> Result<Option<(PathBuf, Self)>, ServiceError> {
        let mut current = start_dir;

        loop {
            let config_path = current.join("sgconfig.yml");
            if config_path.exists() {
                let config = Self::from_file(&config_path)?;
                return Ok(Some((config_path, config)));
            }

            // Also check for sgconfig.yaml
            let config_path = current.join("sgconfig.yaml");
            if config_path.exists() {
                let config = Self::from_file(&config_path)?;
                return Ok(Some((config_path, config)));
            }

            // Move up to parent directory
            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }

        Ok(None)
    }

    /// Resolve all relative paths in the configuration relative to the config file location
    pub fn resolve_paths(&mut self, config_dir: &Path) {
        // Resolve rule directories
        self.rule_dirs = self
            .rule_dirs
            .iter()
            .map(|p| Self::resolve_path(config_dir, p))
            .collect();

        // Resolve test configurations
        for test_config in &mut self.test_configs {
            test_config.test_dir = Self::resolve_path(config_dir, &test_config.test_dir);
        }

        // Resolve utility directories
        self.util_dirs = self
            .util_dirs
            .iter()
            .map(|p| Self::resolve_path(config_dir, p))
            .collect();

        // Resolve custom language library paths
        for lang in self.custom_languages.languages.values_mut() {
            lang.library_path = Self::resolve_path(config_dir, &lang.library_path);
        }
    }

    /// Resolve a potentially relative path against a base directory
    fn resolve_path(base: &Path, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        }
    }

    /// Merge this configuration with another (other takes precedence)
    pub fn merge(self, other: Self) -> Self {
        Self {
            rule_dirs: if other.rule_dirs.is_empty() {
                self.rule_dirs
            } else {
                other.rule_dirs
            },
            test_configs: if other.test_configs.is_empty() {
                self.test_configs
            } else {
                other.test_configs
            },
            util_dirs: if other.util_dirs.is_empty() {
                self.util_dirs
            } else {
                other.util_dirs
            },
            custom_languages: if other.custom_languages.is_empty() {
                self.custom_languages
            } else {
                other.custom_languages
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_basic_config() {
        let yaml = r#"
ruleDirs:
  - ./rules
  - ./team-rules
"#;

        let config = SgConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.rule_dirs.len(), 2);
        assert_eq!(config.rule_dirs[0], PathBuf::from("./rules"));
        assert_eq!(config.rule_dirs[1], PathBuf::from("./team-rules"));
    }

    #[test]
    fn test_parse_full_config() {
        let yaml = r#"
ruleDirs:
  - ./rules
testConfigs:
  - testDir: ./tests
    snapshotDir: ./snapshots
utilDirs:
  - ./utils
customLanguages:
  mylang:
    libraryPath: ./mylang.so
    extensions: [ml, mli]
    expandoChar: _
"#;

        let config = SgConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.rule_dirs.len(), 1);
        assert_eq!(config.test_configs.len(), 1);
        assert_eq!(config.util_dirs.len(), 1);
        assert_eq!(config.custom_languages.languages.len(), 1);

        let mylang = &config.custom_languages.languages["mylang"];
        assert_eq!(mylang.extensions, vec!["ml", "mli"]);
        assert_eq!(mylang.expando_char, Some('_'));
    }

    #[test]
    fn test_discover_config() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("src").join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();

        // Create sgconfig.yml in temp directory
        let config_content = r#"
ruleDirs:
  - ./rules
"#;
        fs::write(temp_dir.path().join("sgconfig.yml"), config_content).unwrap();

        // Test discovery from subdirectory
        let result = SgConfig::discover(&sub_dir).unwrap();
        assert!(result.is_some());

        let (path, config) = result.unwrap();
        assert_eq!(path, temp_dir.path().join("sgconfig.yml"));
        assert_eq!(config.rule_dirs.len(), 1);
    }

    #[test]
    fn test_resolve_paths() {
        let yaml = r#"
ruleDirs:
  - ./rules
  - /absolute/rules
"#;

        let mut config = SgConfig::from_yaml(yaml).unwrap();
        let base_dir = PathBuf::from("/project");

        config.resolve_paths(&base_dir);

        assert_eq!(config.rule_dirs[0], PathBuf::from("/project/rules"));
        assert_eq!(config.rule_dirs[1], PathBuf::from("/absolute/rules"));
    }
}
