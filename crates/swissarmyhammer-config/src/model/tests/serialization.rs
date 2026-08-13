//! The serialized forms of a model configuration.
//!
//! YAML and JSON round trips, the model source and agent source variants,
//! the display of each, and the agent record.

use super::*;

#[test]
fn test_configuration_serialization_yaml() {
    let config = embedding_model_config();

    // Should serialize to YAML correctly
    let yaml = serde_yaml_ng::to_string(&config).expect("Failed to serialize to YAML");
    assert!(yaml.contains("type: llama-embedding"));
    assert!(yaml.contains("quiet: false"));

    // Should deserialize from YAML correctly
    let deserialized: ModelConfig =
        serde_yaml_ng::from_str(&yaml).expect("Failed to deserialize from YAML");
    assert_eq!(
        config.executor_type().unwrap(),
        deserialized.executor_type().unwrap()
    );
    assert_eq!(config.quiet, deserialized.quiet);
}

#[test]
fn test_configuration_serialization_json() {
    let config = embedding_model_config();

    // Should serialize to JSON correctly
    let json = serde_json::to_string(&config).expect("Failed to serialize to JSON");
    assert!(json.contains("\"type\":\"llama-embedding\""));
    assert!(json.contains("\"quiet\":false"));

    // Should deserialize from JSON correctly
    let deserialized: ModelConfig =
        serde_json::from_str(&json).expect("Failed to deserialize from JSON");
    assert_eq!(
        config.executor_type().unwrap(),
        deserialized.executor_type().unwrap()
    );
    assert_eq!(config.quiet, deserialized.quiet);
}

#[test]
fn test_model_source_serialization() {
    let huggingface_source = ModelSource::HuggingFace {
        repo: "test/repo".to_string(),
        filename: Some("model.gguf".to_string()),
        folder: None,
    };

    let json =
        serde_json::to_string(&huggingface_source).expect("Failed to serialize HuggingFace source");
    let deserialized: ModelSource =
        serde_json::from_str(&json).expect("Failed to deserialize HuggingFace source");

    match deserialized {
        ModelSource::HuggingFace { repo, filename, .. } => {
            assert_eq!(repo, "test/repo");
            assert_eq!(filename, Some("model.gguf".to_string()));
        }
        ModelSource::Local { .. } => panic!("Should be HuggingFace source"),
    }

    let local_source = ModelSource::Local {
        filename: PathBuf::from("/path/to/model.gguf"),
        folder: None,
    };

    let json = serde_json::to_string(&local_source).expect("Failed to serialize Local source");
    let deserialized: ModelSource =
        serde_json::from_str(&json).expect("Failed to deserialize Local source");

    match deserialized {
        ModelSource::Local { filename, folder } => {
            assert_eq!(filename, PathBuf::from("/path/to/model.gguf"));
            assert_eq!(folder, None);
        }
        ModelSource::HuggingFace { .. } => panic!("Should be Local source"),
    }
}

#[test]
fn test_model_source_local_with_folder_serialization() {
    // Test serialization of ModelSource::Local with explicit folder
    let local_source_with_folder = ModelSource::Local {
        filename: PathBuf::from("model.gguf"),
        folder: Some(PathBuf::from("/custom/folder")),
    };

    let json = serde_json::to_string(&local_source_with_folder)
        .expect("Failed to serialize Local source with folder");

    let deserialized: ModelSource =
        serde_json::from_str(&json).expect("Failed to deserialize Local source with folder");

    match deserialized {
        ModelSource::Local { filename, folder } => {
            assert_eq!(filename, PathBuf::from("model.gguf"));
            assert_eq!(folder, Some(PathBuf::from("/custom/folder")));
        }
        ModelSource::HuggingFace { .. } => panic!("Should be Local source"),
    }

    // Test that folder field is omitted when None (due to skip_serializing_if)
    let local_source_no_folder = ModelSource::Local {
        filename: PathBuf::from("model.gguf"),
        folder: None,
    };

    let json = serde_json::to_string(&local_source_no_folder)
        .expect("Failed to serialize Local source without folder");

    // The JSON should not contain "folder" field when None
    assert!(!json.contains("folder"));
}

#[test]
fn test_huggingface_folder_deserialization() {
    // Test JSON deserialization with folder field
    let json_with_folder = r#"{
            "HuggingFace": {
                "repo": "unsloth/test-repo",
                "folder": "UD-Q4_K_XL"
            }
        }"#;

    let source: ModelSource = serde_json::from_str(json_with_folder)
        .expect("Failed to deserialize HuggingFace source with folder");

    match source {
        ModelSource::HuggingFace {
            repo,
            filename,
            folder,
        } => {
            assert_eq!(repo, "unsloth/test-repo");
            assert_eq!(filename, None);
            assert_eq!(folder, Some("UD-Q4_K_XL".to_string()));
        }
        _ => panic!("Expected HuggingFace source"),
    }

    // Test JSON deserialization with both filename and folder
    let json_with_both = r#"{
            "HuggingFace": {
                "repo": "unsloth/test-repo",
                "filename": "model.gguf",
                "folder": "UD-Q4_K_XL"
            }
        }"#;

    let source: ModelSource = serde_json::from_str(json_with_both)
        .expect("Failed to deserialize HuggingFace source with both filename and folder");

    match source {
        ModelSource::HuggingFace {
            repo,
            filename,
            folder,
        } => {
            assert_eq!(repo, "unsloth/test-repo");
            assert_eq!(filename, Some("model.gguf".to_string()));
            assert_eq!(folder, Some("UD-Q4_K_XL".to_string()));
        }
        _ => panic!("Expected HuggingFace source"),
    }
}

#[test]
fn test_model_source_variants() {
    // Test all ModelSource variants exist and have correct Debug output
    assert_eq!(format!("{:?}", ModelConfigSource::Builtin), "Builtin");
    assert_eq!(format!("{:?}", ModelConfigSource::Project), "Project");
    assert_eq!(format!("{:?}", ModelConfigSource::User), "User");
}

#[test]
fn test_model_source_equality() {
    assert_eq!(ModelConfigSource::Builtin, ModelConfigSource::Builtin);
    assert_eq!(ModelConfigSource::Project, ModelConfigSource::Project);
    assert_eq!(ModelConfigSource::User, ModelConfigSource::User);

    assert_ne!(ModelConfigSource::Builtin, ModelConfigSource::Project);
    assert_ne!(ModelConfigSource::Builtin, ModelConfigSource::User);
    assert_ne!(ModelConfigSource::Project, ModelConfigSource::User);
}

#[test]
fn test_model_source_display_emoji() {
    assert_eq!(ModelConfigSource::Builtin.display_emoji(), "📦 Built-in");
    assert_eq!(ModelConfigSource::Project.display_emoji(), "📁 Project");
    assert_eq!(ModelConfigSource::User.display_emoji(), "👤 User");
}

#[test]
fn test_agent_source_serialization() {
    // Test serde serialization with kebab-case
    let builtin = ModelConfigSource::Builtin;
    let json = serde_json::to_string(&builtin).expect("Failed to serialize Builtin");
    assert_eq!(json, "\"builtin\"");

    let project = ModelConfigSource::Project;
    let json = serde_json::to_string(&project).expect("Failed to serialize Project");
    assert_eq!(json, "\"project\"");

    let user = ModelConfigSource::User;
    let json = serde_json::to_string(&user).expect("Failed to serialize User");
    assert_eq!(json, "\"user\"");
}

#[test]
fn test_agent_source_deserialization() {
    let builtin: ModelConfigSource =
        serde_json::from_str("\"builtin\"").expect("Failed to deserialize builtin");
    assert_eq!(builtin, ModelConfigSource::Builtin);

    let project: ModelConfigSource =
        serde_json::from_str("\"project\"").expect("Failed to deserialize project");
    assert_eq!(project, ModelConfigSource::Project);

    let user: ModelConfigSource =
        serde_json::from_str("\"user\"").expect("Failed to deserialize user");
    assert_eq!(user, ModelConfigSource::User);
}

#[test]
fn test_model_error_display() {
    let not_found = ModelError::NotFound("test-agent".to_string());
    assert_eq!(format!("{}", not_found), "model 'test-agent' not found");

    let invalid_path = ModelError::InvalidPath(PathBuf::from("/invalid/path"));
    assert!(format!("{}", invalid_path).contains("invalid model path"));
    assert!(format!("{}", invalid_path).contains("/invalid/path"));
}

#[test]
fn test_model_error_from_io_error() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let model_error: ModelError = io_error.into();

    match model_error {
        ModelError::IoError(_) => {} // Expected
        _ => panic!("Should convert to IoError variant"),
    }
}

#[test]
fn test_model_error_from_serde_yaml_ng_error() {
    let invalid_yaml = "invalid: yaml: content: [unclosed";
    let yaml_error = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(invalid_yaml)
        .expect_err("Should fail to parse invalid YAML");
    let model_error: ModelError = yaml_error.into();

    match model_error {
        ModelError::ParseError(_) => {} // Expected
        _ => panic!("Should convert to ParseError variant"),
    }
}

#[test]
fn test_agent_info_creation() {
    let agent_info = ModelInfo {
        name: "test-agent".to_string(),
        content: "agent: config".to_string(),
        source: ModelConfigSource::Builtin,
        description: Some("Test agent description".to_string()),
    };

    assert_eq!(agent_info.name, "test-agent");
    assert_eq!(agent_info.content, "agent: config");
    assert_eq!(agent_info.source, ModelConfigSource::Builtin);
    assert_eq!(
        agent_info.description,
        Some("Test agent description".to_string())
    );
}

#[test]
fn test_agent_info_equality() {
    let agent1 = ModelInfo {
        name: "test".to_string(),
        content: "config".to_string(),
        source: ModelConfigSource::Builtin,
        description: None,
    };

    let agent2 = ModelInfo {
        name: "test".to_string(),
        content: "config".to_string(),
        source: ModelConfigSource::Builtin,
        description: None,
    };

    let agent3 = ModelInfo {
        name: "different".to_string(),
        content: "config".to_string(),
        source: ModelConfigSource::Builtin,
        description: None,
    };

    assert_eq!(agent1, agent2);
    assert_ne!(agent1, agent3);
}

#[test]
fn test_agent_info_serialization() {
    let agent_info = ModelInfo {
        name: "test-agent".to_string(),
        content: "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false".to_string(),
        source: ModelConfigSource::User,
        description: Some("A test agent".to_string()),
    };

    let json = serde_json::to_string(&agent_info).expect("Failed to serialize ModelInfo");
    let deserialized: ModelInfo =
        serde_json::from_str(&json).expect("Failed to deserialize ModelInfo");

    assert_eq!(agent_info, deserialized);
}
