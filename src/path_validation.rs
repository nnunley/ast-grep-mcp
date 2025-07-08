use crate::errors::ServiceError;
use std::path::{Path, PathBuf};

/// Validates that a path is within one of the allowed root directories
pub fn validate_path_within_roots(
    path: &Path,
    root_directories: &[PathBuf],
) -> Result<PathBuf, ServiceError> {
    // If path doesn't exist, check if it would be within roots when created
    if !path.exists() {
        // For non-existent paths, check if any root is a prefix
        for root in root_directories {
            if let Ok(canonical_root) = root.canonicalize() {
                // Check if the path would be under this root
                if let Ok(abs_path) = path.canonicalize().or_else(|_| {
                    // If canonicalize fails, try to make it absolute
                    if path.is_absolute() {
                        Ok(path.to_path_buf())
                    } else {
                        std::env::current_dir().map(|cwd| cwd.join(path))
                    }
                }) {
                    if abs_path.starts_with(&canonical_root) {
                        return Ok(abs_path);
                    }
                }
            }
        }
        return Err(ServiceError::Internal(
            "Path is outside allowed directories".to_string(),
        ));
    }

    // For existing paths, canonicalize and check
    let canonical_path = path
        .canonicalize()
        .map_err(|_| ServiceError::FileNotFound(path.to_path_buf()))?;

    // Check if the canonical path is within any of the root directories
    for root in root_directories {
        let canonical_root = root
            .canonicalize()
            .map_err(|_| ServiceError::FileNotFound(root.clone()))?;

        if canonical_path.starts_with(&canonical_root) {
            return Ok(canonical_path);
        }
    }

    Err(ServiceError::Internal(
        "Path is outside allowed directories".to_string(),
    ))
}

/// Validates a path pattern and ensures it cannot escape root directories
pub fn validate_path_pattern(pattern: &str) -> Result<String, ServiceError> {
    // Check for directory traversal attempts
    if pattern.contains("../") || pattern.contains("..\\") || pattern == ".." {
        return Err(ServiceError::Internal(
            "Path traversal patterns are not allowed".to_string(),
        ));
    }

    // Absolute paths (including Windows drive letters) will be validated against roots in the search logic

    Ok(pattern.to_string())
}

/// Resolves a path pattern to actual paths within allowed roots
pub fn resolve_path_pattern(
    pattern: &str,
    root_directories: &[PathBuf],
) -> Result<Vec<PathBuf>, ServiceError> {
    let mut resolved_paths = Vec::new();

    // Validate the pattern first
    let validated_pattern = validate_path_pattern(pattern)?;

    // If it's an absolute path, validate it's within roots
    if validated_pattern.starts_with('/') {
        let path = PathBuf::from(&validated_pattern);
        let validated_path = validate_path_within_roots(&path, root_directories)?;
        resolved_paths.push(validated_path);
    } else {
        // For relative patterns, resolve within each root
        for root in root_directories {
            let full_path = root.join(&validated_pattern);
            if let Ok(canonical) = full_path.canonicalize() {
                // Double-check it's still within the root after canonicalization
                if let Ok(canonical_root) = root.canonicalize() {
                    if canonical.starts_with(&canonical_root) {
                        resolved_paths.push(canonical);
                    }
                }
            }
        }
    }

    Ok(resolved_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_path_within_roots() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        let roots = vec![root.clone()];

        // Valid path within root
        let valid_path = root.join("subdir/file.txt");
        std::fs::create_dir_all(valid_path.parent().unwrap()).unwrap();
        std::fs::write(&valid_path, "test").unwrap();
        assert!(validate_path_within_roots(&valid_path, &roots).is_ok());

        // Path outside root - create another temp dir to ensure it's outside
        let other_temp = TempDir::new().unwrap();
        let outside_path = other_temp.path().join("outside.txt");
        std::fs::write(&outside_path, "test").unwrap();
        assert!(validate_path_within_roots(&outside_path, &roots).is_err());
    }

    #[test]
    fn test_validate_path_pattern() {
        // Valid patterns
        assert!(validate_path_pattern("**/*.js").is_ok());
        assert!(validate_path_pattern("src/main.rs").is_ok());
        assert!(validate_path_pattern("./test.txt").is_ok());

        // Invalid patterns with directory traversal
        assert!(validate_path_pattern("../escape").is_err());
        assert!(validate_path_pattern("foo/../../../etc/passwd").is_err());
        assert!(validate_path_pattern("..").is_err());
    }

    #[test]
    fn test_resolve_path_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        let roots = vec![root.clone()];

        // Create test file
        let test_file = root.join("test.txt");
        std::fs::write(&test_file, "test").unwrap();

        // Relative pattern
        let resolved = resolve_path_pattern("test.txt", &roots).unwrap();
        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].ends_with("test.txt"));

        // Invalid pattern
        assert!(resolve_path_pattern("../escape.txt", &roots).is_err());
    }
}
