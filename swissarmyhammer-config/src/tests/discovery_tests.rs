//! Comprehensive tests for the configuration file discovery system

use crate::discovery::{ConfigFormat, ConfigScope, FileDiscovery};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_file_discovery_integration() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    // Create both global and project directories
    let global_dir = temp_dir.path().join("home").join(".swissarmyhammer");
    let project_dir = temp_dir.path().join("project").join(".swissarmyhammer");
    fs::create_dir_all(&global_dir).unwrap();
    fs::create_dir_all(&project_dir).unwrap();

    // Create config files with different formats and names
    fs::write(global_dir.join("sah.toml"), "global_toml = true").unwrap();
    fs::write(global_dir.join("swissarmyhammer.yaml"), "global_yaml: true").unwrap();
    fs::write(project_dir.join("sah.json"), r#"{"project_json": true}"#).unwrap();
    fs::write(
        project_dir.join("swissarmyhammer.toml"),
        "project_toml = true",
    )
    .unwrap();

    // Change to project directory
    std::env::set_current_dir(temp_dir.path().join("project")).unwrap();

    // Create discovery with custom directories for testing
    let discovery = FileDiscovery::with_directories(Some(project_dir), Some(global_dir));

    let files = discovery.discover_all();

    // Restore directory
    std::env::set_current_dir(original_dir).unwrap();

    // Should find 4 files
    assert_eq!(files.len(), 4);

    // Check that all files are found
    let global_toml = files
        .iter()
        .find(|f| f.path.file_name().unwrap() == "sah.toml" && f.scope == ConfigScope::Global);
    let global_yaml = files.iter().find(|f| {
        f.path.file_name().unwrap() == "swissarmyhammer.yaml" && f.scope == ConfigScope::Global
    });
    let project_json = files
        .iter()
        .find(|f| f.path.file_name().unwrap() == "sah.json" && f.scope == ConfigScope::Project);
    let project_toml = files.iter().find(|f| {
        f.path.file_name().unwrap() == "swissarmyhammer.toml" && f.scope == ConfigScope::Project
    });

    assert!(global_toml.is_some());
    assert!(global_yaml.is_some());
    assert!(project_json.is_some());
    assert!(project_toml.is_some());

    // Check formats
    assert_eq!(global_toml.unwrap().format, ConfigFormat::Toml);
    assert_eq!(global_yaml.unwrap().format, ConfigFormat::Yaml);
    assert_eq!(project_json.unwrap().format, ConfigFormat::Json);
    assert_eq!(project_toml.unwrap().format, ConfigFormat::Toml);

    // Check priority ordering (global files should come before project files)
    let first_global = files
        .iter()
        .position(|f| f.scope == ConfigScope::Global)
        .unwrap();
    let first_project = files
        .iter()
        .position(|f| f.scope == ConfigScope::Project)
        .unwrap();
    assert!(first_global < first_project);
}

#[test]
fn test_discovery_with_missing_directories() {
    let temp_dir = TempDir::new().unwrap();

    // Create discovery with non-existent directories
    let discovery = FileDiscovery::with_directories(
        Some(temp_dir.path().join("nonexistent_project")),
        Some(temp_dir.path().join("nonexistent_global")),
    );

    let files = discovery.discover_all();

    // Should handle missing directories gracefully
    assert!(files.is_empty());
}

#[test]
fn test_discovery_ignores_invalid_files() {
    let temp_dir = TempDir::new().unwrap();
    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Create valid config files
    fs::write(sah_dir.join("sah.toml"), "valid = true").unwrap();
    fs::write(sah_dir.join("swissarmyhammer.yaml"), "valid: true").unwrap();

    // Create invalid files that should be ignored
    fs::write(sah_dir.join("config.toml"), "invalid = true").unwrap(); // Wrong name
    fs::write(sah_dir.join("sah.txt"), "invalid = true").unwrap(); // Wrong extension
    fs::write(sah_dir.join("other.json"), r#"{"invalid": true}"#).unwrap(); // Wrong name

    let discovery = FileDiscovery::with_directories(Some(sah_dir), None);

    let files = discovery.discover_all();

    // Should only find the 2 valid files
    assert_eq!(files.len(), 2);

    let valid_names: Vec<&str> = files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_str().unwrap())
        .collect();

    assert!(valid_names.contains(&"sah.toml"));
    assert!(valid_names.contains(&"swissarmyhammer.yaml"));
    assert!(!valid_names.contains(&"config.toml"));
    assert!(!valid_names.contains(&"sah.txt"));
    assert!(!valid_names.contains(&"other.json"));
}

#[test]
fn test_discovery_all_file_formats_and_names() {
    let temp_dir = TempDir::new().unwrap();
    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Create all possible valid config file combinations
    let test_files = [
        ("sah.toml", "test = \"toml\""),
        ("sah.yaml", "test: yaml"),
        ("sah.yml", "test: yml"),
        ("sah.json", r#"{"test": "json"}"#),
        ("swissarmyhammer.toml", "test = \"toml_long\""),
        ("swissarmyhammer.yaml", "test: yaml_long"),
        ("swissarmyhammer.yml", "test: yml_long"),
        ("swissarmyhammer.json", r#"{"test": "json_long"}"#),
    ];

    for (filename, content) in &test_files {
        fs::write(sah_dir.join(filename), content).unwrap();
    }

    let discovery = FileDiscovery::with_directories(Some(sah_dir), None);

    let files = discovery.discover_all();

    // Should find all 8 files
    assert_eq!(files.len(), 8);

    // Check that all expected files are found with correct formats
    for (expected_filename, _) in &test_files {
        let found = files
            .iter()
            .find(|f| f.path.file_name().unwrap().to_str().unwrap() == *expected_filename);
        assert!(found.is_some(), "File {} not found", expected_filename);

        let file = found.unwrap();
        let expected_format = match expected_filename {
            name if name.ends_with(".toml") => ConfigFormat::Toml,
            name if name.ends_with(".yaml") || name.ends_with(".yml") => ConfigFormat::Yaml,
            name if name.ends_with(".json") => ConfigFormat::Json,
            _ => panic!("Unexpected file extension"),
        };

        assert_eq!(
            file.format, expected_format,
            "Wrong format for {}",
            expected_filename
        );
        assert_eq!(file.scope, ConfigScope::Project);
    }
}

#[test]
fn test_discovery_priority_ordering() {
    let temp_dir = TempDir::new().unwrap();

    let global_dir = temp_dir.path().join("global");
    let project_dir = temp_dir.path().join("project");
    fs::create_dir_all(&global_dir).unwrap();
    fs::create_dir_all(&project_dir).unwrap();

    // Create identical filenames in both directories
    fs::write(global_dir.join("sah.toml"), "scope = \"global\"").unwrap();
    fs::write(project_dir.join("sah.toml"), "scope = \"project\"").unwrap();
    fs::write(global_dir.join("swissarmyhammer.yaml"), "scope: global").unwrap();
    fs::write(project_dir.join("swissarmyhammer.yaml"), "scope: project").unwrap();

    let discovery = FileDiscovery::with_directories(Some(project_dir), Some(global_dir));

    let files = discovery.discover_all();

    assert_eq!(files.len(), 4);

    // Files should be sorted by priority (ascending)
    for i in 1..files.len() {
        assert!(
            files[i - 1].priority <= files[i].priority,
            "Files not sorted by priority: {} > {}",
            files[i - 1].priority,
            files[i].priority
        );
    }

    // Global files should come first, then project files
    let global_files: Vec<_> = files
        .iter()
        .filter(|f| f.scope == ConfigScope::Global)
        .collect();
    let project_files: Vec<_> = files
        .iter()
        .filter(|f| f.scope == ConfigScope::Project)
        .collect();

    assert_eq!(global_files.len(), 2);
    assert_eq!(project_files.len(), 2);

    // All global files should have lower priority than project files
    for global_file in &global_files {
        for project_file in &project_files {
            assert!(global_file.priority < project_file.priority);
        }
    }
}

#[test]
fn test_edge_cases() {
    let temp_dir = TempDir::new().unwrap();

    // Test with empty discovery
    let empty_discovery = FileDiscovery::with_directories(None, None);
    assert!(empty_discovery.discover_all().is_empty());

    // Test with directory that exists but has no config files
    let empty_dir = temp_dir.path().join("empty");
    fs::create_dir_all(&empty_dir).unwrap();

    let discovery_with_empty = FileDiscovery::with_directories(Some(empty_dir), None);
    assert!(discovery_with_empty.discover_all().is_empty());

    // Test with file that exists but is not readable (simulate with directory)
    let bad_dir = temp_dir.path().join("bad");
    fs::create_dir_all(&bad_dir).unwrap();
    fs::create_dir_all(bad_dir.join("sah.toml")).unwrap(); // Create directory with config name

    let discovery_with_bad = FileDiscovery::with_directories(Some(bad_dir), None);

    // Should handle gracefully and return empty (directory instead of file)
    assert!(discovery_with_bad.discover_all().is_empty());
}

#[test]
fn test_real_directory_detection() {
    // This tests the actual directory resolution methods
    let original_dir = std::env::current_dir().unwrap();
    let temp_dir = TempDir::new().unwrap();

    // Create a project directory structure
    let project_root = temp_dir.path().join("test_project");
    let sah_dir = project_root.join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();
    fs::write(sah_dir.join("sah.toml"), "test = true").unwrap();

    // Change to project directory
    std::env::set_current_dir(&project_root).unwrap();

    // Create discovery using the actual resolution methods
    let discovery = FileDiscovery::new();
    let files = discovery.discover_all();

    // Should find the config file
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].scope, ConfigScope::Project);
    assert_eq!(files[0].format, ConfigFormat::Toml);

    // Restore directory
    std::env::set_current_dir(original_dir).unwrap();
}
