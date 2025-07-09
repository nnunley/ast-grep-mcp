//! Test sgconfig.yml integration with ServiceConfig

use ast_grep_mcp::config::ServiceConfig;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_service_config_with_sgconfig() {
    let temp_dir = TempDir::new().unwrap();

    // Create sgconfig.yml
    let sg_config_content = r#"
ruleDirs:
  - ./rules
  - ./team-rules
utilDirs:
  - ./utils
"#;

    fs::write(temp_dir.path().join("sgconfig.yml"), sg_config_content).unwrap();

    // Create directories
    fs::create_dir_all(temp_dir.path().join("rules")).unwrap();
    fs::create_dir_all(temp_dir.path().join("team-rules")).unwrap();
    fs::create_dir_all(temp_dir.path().join("utils")).unwrap();

    // Create config and load sgconfig.yml
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        rules_directory: temp_dir.path().join(".ast-grep-rules"),
        ..Default::default()
    };

    let config_with_sg = config.with_sg_config(None);

    // Check that sgconfig was loaded
    assert!(config_with_sg.sg_config_path.is_some());
    assert_eq!(config_with_sg.additional_rule_dirs.len(), 2);
    assert_eq!(config_with_sg.util_dirs.len(), 1);

    // Check all_rule_directories includes both primary and additional
    let all_dirs = config_with_sg.all_rule_directories();
    assert_eq!(all_dirs.len(), 3); // primary + 2 additional
    assert!(all_dirs.contains(&config_with_sg.rules_directory));
    assert!(all_dirs.contains(&temp_dir.path().join("rules")));
    assert!(all_dirs.contains(&temp_dir.path().join("team-rules")));
}

#[test]
fn test_service_config_with_nested_sgconfig() {
    let temp_dir = TempDir::new().unwrap();
    let sub_dir = temp_dir.path().join("src").join("components");
    fs::create_dir_all(&sub_dir).unwrap();

    // Create sgconfig.yml at root
    let sg_config_content = r#"
ruleDirs:
  - ./project-rules
"#;

    fs::write(temp_dir.path().join("sgconfig.yml"), sg_config_content).unwrap();
    fs::create_dir_all(temp_dir.path().join("project-rules")).unwrap();

    // Create config starting from subdirectory
    let config = ServiceConfig {
        root_directories: vec![sub_dir.clone()],
        ..Default::default()
    };

    let config_with_sg = config.with_sg_config(None);

    // Should discover sgconfig.yml from parent directories
    assert!(config_with_sg.sg_config_path.is_some());
    assert_eq!(config_with_sg.additional_rule_dirs.len(), 1);
    assert!(config_with_sg.additional_rule_dirs[0].ends_with("project-rules"));
}

#[test]
fn test_service_config_with_explicit_path() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&config_dir).unwrap();

    // Create sgconfig.yml in config directory
    let sg_config_content = r#"
ruleDirs:
  - ../rules
"#;

    let config_path = config_dir.join("sgconfig.yml");
    fs::write(&config_path, sg_config_content).unwrap();
    fs::create_dir_all(temp_dir.path().join("rules")).unwrap();

    // Create config with explicit path
    let config = ServiceConfig::default();
    let config_with_sg = config.with_sg_config(Some(&config_path));

    // Should use the explicit path
    assert_eq!(config_with_sg.sg_config_path, Some(config_path));
    assert_eq!(config_with_sg.additional_rule_dirs.len(), 1);
    // Path should be resolved relative to config file location
    assert!(config_with_sg.additional_rule_dirs[0].ends_with("rules"));
}

#[test]
fn test_service_config_without_sgconfig() {
    let temp_dir = TempDir::new().unwrap();

    // No sgconfig.yml file exists
    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };

    let config_with_sg = config.with_sg_config(None);

    // Should work without sgconfig.yml
    assert!(config_with_sg.sg_config_path.is_none());
    assert!(config_with_sg.additional_rule_dirs.is_empty());
    assert!(config_with_sg.util_dirs.is_empty());

    // all_rule_directories should only contain the primary directory
    let all_dirs = config_with_sg.all_rule_directories();
    assert_eq!(all_dirs.len(), 1);
    assert_eq!(all_dirs[0], config_with_sg.rules_directory);
}

#[test]
fn test_service_config_with_absolute_paths() {
    let temp_dir = TempDir::new().unwrap();
    let abs_rules_dir = temp_dir.path().join("absolute-rules");
    fs::create_dir_all(&abs_rules_dir).unwrap();

    // Create sgconfig.yml with both relative and absolute paths
    let sg_config_content = format!(
        r#"
ruleDirs:
  - ./relative-rules
  - {}
"#,
        abs_rules_dir.display()
    );

    fs::write(temp_dir.path().join("sgconfig.yml"), sg_config_content).unwrap();
    fs::create_dir_all(temp_dir.path().join("relative-rules")).unwrap();

    let config = ServiceConfig {
        root_directories: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };

    let config_with_sg = config.with_sg_config(None);

    assert_eq!(config_with_sg.additional_rule_dirs.len(), 2);
    // Check that absolute path is preserved
    assert!(config_with_sg.additional_rule_dirs.contains(&abs_rules_dir));
    // Check that relative path is resolved
    assert!(
        config_with_sg
            .additional_rule_dirs
            .iter()
            .any(|p| p.ends_with("relative-rules"))
    );
}
