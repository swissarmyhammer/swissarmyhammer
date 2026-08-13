//! Reading model files off disk.
//!
//! The name a path yields, the content one file gives, and what a walk of
//! a directory keeps and drops.

use super::*;

#[test]
fn test_extract_model_name_normal() {
    // Standard filename should extract stem without extension
    let path = Path::new("/models/my-agent.yaml");
    let name = ModelManager::extract_model_name(path).expect("should extract name");
    assert_eq!(name, "my-agent");
}

#[test]
fn test_extract_model_name_nested_path() {
    // Deeply nested path should still extract just the file stem
    let path = Path::new("/a/b/c/deep-model.yaml");
    let name = ModelManager::extract_model_name(path).expect("should extract name");
    assert_eq!(name, "deep-model");
}

#[test]
fn test_extract_model_name_no_extension() {
    // File without extension should still extract the full filename as stem
    let path = Path::new("/models/no-ext");
    let name = ModelManager::extract_model_name(path).expect("should extract name");
    assert_eq!(name, "no-ext");
}

#[test]
fn test_extract_model_name_root_path() {
    // Root path "/" has no file stem and should return InvalidPath
    let result = ModelManager::extract_model_name(Path::new("/"));
    assert!(
        result.is_err(),
        "Root path should fail to extract model name"
    );
    match result.unwrap_err() {
        ModelError::InvalidPath(_) => {} // expected
        other => panic!("Expected InvalidPath, got: {:?}", other),
    }
}

#[test]
fn test_extract_model_name_dotfile() {
    // Hidden file like ".hidden.yaml" should extract ".hidden" as stem
    let path = Path::new("/models/.hidden.yaml");
    let name = ModelManager::extract_model_name(path).expect("should extract name");
    assert_eq!(name, ".hidden");
}

#[test]
fn test_read_model_content_missing_file() {
    // Reading a non-existent file should return IoError
    let result = ModelManager::read_model_content(Path::new("/no/such/file.yaml"));
    assert!(result.is_err());
    match result.unwrap_err() {
        ModelError::IoError(_) => {} // expected
        other => panic!("Expected IoError, got: {:?}", other),
    }
}

#[test]
fn test_read_model_content_success() {
    // Reading an existing file should return its content
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("test.yaml");
    std::fs::write(&file_path, "executor:\n  type: llama-embedding\n").expect("write");

    let content = ModelManager::read_model_content(&file_path).expect("should read");
    assert!(content.contains("llama-embedding"));
}

#[test]
fn test_process_directory_entries_mixed_success_and_failure() {
    // Directory with valid YAML, invalid YAML, and non-YAML files should
    // report correct success/failure counts and only return valid models.
    use std::fs;
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");

    // Valid model file
    let valid_content = "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n";
    fs::write(temp_dir.path().join("good-model.yaml"), valid_content).expect("write valid");

    // Invalid YAML model file (parseable YAML but invalid ModelConfig)
    fs::write(
        temp_dir.path().join("bad-model.yaml"),
        "this_is_not: a_valid_model_config\n",
    )
    .expect("write invalid");

    // Non-YAML file (should be silently skipped)
    fs::write(temp_dir.path().join("readme.txt"), "ignore me").expect("write txt");

    let entries = std::fs::read_dir(temp_dir.path()).expect("read dir");
    let (models, success, failed) =
        ModelManager::process_directory_entries(entries, &ModelConfigSource::Project);

    assert_eq!(success, 1, "Should have 1 successful model");
    assert_eq!(failed, 1, "Should have 1 failed model (bad YAML)");
    assert_eq!(models.len(), 1, "Should return 1 model");
    assert_eq!(models[0].name, "good-model");
    assert_eq!(models[0].source, ModelConfigSource::Project);
}

#[test]
fn test_process_directory_entries_all_valid() {
    // Directory with only valid model files should load all of them
    use std::fs;
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");

    let content = "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n";
    fs::write(temp_dir.path().join("model-a.yaml"), content).expect("write a");
    fs::write(temp_dir.path().join("model-b.yaml"), content).expect("write b");

    let entries = std::fs::read_dir(temp_dir.path()).expect("read dir");
    let (models, success, failed) =
        ModelManager::process_directory_entries(entries, &ModelConfigSource::User);

    assert_eq!(success, 2);
    assert_eq!(failed, 0);
    assert_eq!(models.len(), 2);
}

#[test]
fn test_process_directory_entries_empty_directory() {
    // Empty directory should return zero models and zero counts
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");

    let entries = std::fs::read_dir(temp_dir.path()).expect("read dir");
    let (models, success, failed) =
        ModelManager::process_directory_entries(entries, &ModelConfigSource::Project);

    assert_eq!(success, 0);
    assert_eq!(failed, 0);
    assert!(models.is_empty());
}

#[test]
fn test_process_directory_entries_only_non_yaml() {
    // Directory containing only non-YAML files should skip them all
    use std::fs;
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");

    fs::write(temp_dir.path().join("readme.md"), "# Hello").expect("write md");
    fs::write(temp_dir.path().join("config.json"), "{}").expect("write json");
    fs::write(temp_dir.path().join("script.sh"), "#!/bin/sh").expect("write sh");

    let entries = std::fs::read_dir(temp_dir.path()).expect("read dir");
    let (models, success, failed) =
        ModelManager::process_directory_entries(entries, &ModelConfigSource::Project);

    assert_eq!(success, 0);
    assert_eq!(failed, 0);
    assert!(models.is_empty());
}

#[test]
fn test_is_yaml_file_extensions() {
    // Only .yaml extension files that are actual files should match
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");

    let yaml_path = temp_dir.path().join("model.yaml");
    std::fs::write(&yaml_path, "content").expect("write");

    let txt_path = temp_dir.path().join("model.txt");
    std::fs::write(&txt_path, "content").expect("write");

    let yml_path = temp_dir.path().join("model.yml");
    std::fs::write(&yml_path, "content").expect("write");

    assert!(ModelManager::is_yaml_file(&yaml_path));
    assert!(!ModelManager::is_yaml_file(&txt_path));
    assert!(!ModelManager::is_yaml_file(&yml_path)); // only .yaml, not .yml
}

#[test]
fn test_load_models_from_dir_end_to_end_mixed() {
    // Full pipeline: create a temp directory with valid/invalid files,
    // call load_models_from_dir, and verify results.
    use std::fs;
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");

    // Valid model with description
    let content_with_desc = "---\ndescription: \"My custom model\"\n---\nexecutor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n";
    fs::write(temp_dir.path().join("custom.yaml"), content_with_desc).expect("write");

    // Valid model without description
    let content_no_desc = "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: true\n";
    fs::write(temp_dir.path().join("plain.yaml"), content_no_desc).expect("write");

    // Invalid model
    fs::write(temp_dir.path().join("broken.yaml"), "not: valid: model").expect("write");

    // Non-YAML
    fs::write(temp_dir.path().join("notes.txt"), "skip me").expect("write");

    let result = ModelManager::load_models_from_dir(temp_dir.path(), ModelConfigSource::Project);
    assert!(result.is_ok(), "Should succeed: {:?}", result);

    let models = result.unwrap();
    // 2 valid YAML files out of 4 total
    assert_eq!(models.len(), 2, "Should load 2 valid models");

    let custom = models.iter().find(|m| m.name == "custom");
    assert!(custom.is_some(), "Should find 'custom' model");
    assert_eq!(
        custom.unwrap().description,
        Some("My custom model".to_string())
    );

    let plain = models.iter().find(|m| m.name == "plain");
    assert!(plain.is_some(), "Should find 'plain' model");
    assert_eq!(plain.unwrap().description, None);
}
