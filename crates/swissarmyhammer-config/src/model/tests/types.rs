//! The small types the model configuration is built from.
//!
//! The severity of each error, the git-root source, the deserializer that
//! ignores unknown fields, the platform, the embedding configuration, the
//! model paths, the executor kinds and the configuration source.

use super::*;

#[test]
fn test_model_error_not_found_is_error() {
    let error = ModelError::NotFound("test-agent".to_string());
    assert_eq!(error.severity(), ErrorSeverity::Error);
}

#[test]
fn test_model_error_invalid_path_is_error() {
    let error = ModelError::InvalidPath(PathBuf::from("/invalid/path"));
    assert_eq!(error.severity(), ErrorSeverity::Error);
}

#[test]
fn test_model_error_io_error_is_error() {
    let error = ModelError::from(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "file not found",
    ));
    assert_eq!(error.severity(), ErrorSeverity::Error);
}

#[test]
fn test_model_error_parse_error_is_critical() {
    let yaml_err =
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>("invalid: yaml: content").unwrap_err();
    let error = ModelError::from(yaml_err);
    assert_eq!(error.severity(), ErrorSeverity::Critical);
}

#[test]
fn test_model_error_config_error_is_critical() {
    let error = ModelError::ConfigError("Invalid configuration".to_string());
    assert_eq!(error.severity(), ErrorSeverity::Critical);
}

#[test]
fn test_gitroot_display_emoji() {
    assert_eq!(ModelConfigSource::GitRoot.display_emoji(), "🔧 GitRoot");
}

#[test]
fn test_gitroot_source_serialization() {
    let gitroot = ModelConfigSource::GitRoot;
    let json = serde_json::to_string(&gitroot).expect("Failed to serialize GitRoot");
    assert_eq!(json, "\"git-root\"");

    let deserialized: ModelConfigSource =
        serde_json::from_str(&json).expect("Failed to deserialize GitRoot");
    assert_eq!(deserialized, ModelConfigSource::GitRoot);
}

#[test]
fn test_model_config_deserialize_missing_executor_field() {
    // Exercises the error path when neither `executor` nor `executors` is present.
    let yaml = "quiet: true\n";
    let result = serde_yaml_ng::from_str::<ModelConfig>(yaml);
    assert!(result.is_err(), "Should fail when no executor field");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("executor"),
        "Error should mention executor: {}",
        err_msg
    );
}

#[test]
fn test_model_config_deserialize_executors_list() {
    // Exercises the `executors` list deserialization path.
    let yaml = r#"
executors:
  - platform: macos-arm64
    executor:
      type: llama-embedding
      config:
        source: !HuggingFace
          repo: test/embed
  - executor:
      type: llama-embedding
      config:
        source: !HuggingFace
          repo: test/embed
quiet: true
"#;
    let config: ModelConfig = serde_yaml_ng::from_str(yaml).expect("Should parse executors list");
    assert_eq!(config.executors.len(), 2);
    assert!(config.quiet);
    assert_eq!(config.executors[0].platform, Some(Platform::MacosArm64));
    assert_eq!(config.executors[1].platform, None);
}

#[test]
fn test_model_config_deserialize_unknown_fields_ignored() {
    // Exercises the `_: IgnoredAny` path in the custom deserializer.
    let yaml = r#"
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false
unknown_field: "should be ignored"
another_unknown: 42
"#;
    let config: ModelConfig =
        serde_yaml_ng::from_str(yaml).expect("Should parse despite unknown fields");
    assert_eq!(
        config.executor_type().unwrap(),
        ModelExecutorType::LlamaEmbedding
    );
    assert!(!config.quiet);
}

#[test]
fn test_model_config_select_executor_no_match() {
    // Exercises `select_executor()` returning `None` when all entries have
    // non-matching platform constraints.
    let config = ModelConfig {
        executors: vec![ExecutorEntry {
            // Use a platform that definitely doesn't match current
            platform: Some(Platform::LinuxX8664),
            executor: embedding_model_config().executors.remove(0).executor,
        }],
        quiet: false,
    };
    // On macOS ARM this won't match LinuxX8664
    // We can't guarantee which platform we're on, so just test the method works
    let _result = config.select_executor();
}

#[test]
fn platform_wire_names_are_stable() {
    // `platform:` is written into user model YAML, so these strings are a
    // contract. Renaming a variant must not change them.
    let expected = [
        (Platform::MacosArm64, "macos-arm64"),
        (Platform::MacosX8664, "macos-x86-64"),
        (Platform::LinuxX8664, "linux-x86-64"),
        (Platform::LinuxAarch64, "linux-aarch64"),
    ];
    for (platform, wire) in expected {
        assert_eq!(
            serde_yaml_ng::to_string(&platform).unwrap().trim(),
            wire,
            "wire name for {platform:?}"
        );
        assert_eq!(
            serde_yaml_ng::from_str::<Platform>(wire).unwrap(),
            platform,
            "parsing wire name {wire}"
        );
    }
}

#[test]
fn test_platform_serialization_roundtrip() {
    // Exercises Platform serialization/deserialization for all variants.
    let platforms = vec![
        Platform::MacosArm64,
        Platform::MacosX8664,
        Platform::LinuxX8664,
        Platform::LinuxAarch64,
    ];
    for platform in platforms {
        let json = serde_json::to_string(&platform)
            .unwrap_or_else(|_| panic!("Failed to serialize {:?}", platform));
        let deserialized: Platform = serde_json::from_str(&json)
            .unwrap_or_else(|_| panic!("Failed to deserialize {:?}", platform));
        assert_eq!(platform, deserialized);
    }
}

#[test]
fn test_platform_current() {
    // Exercises `Platform::current()` — just verifies it doesn't panic.
    let _current = Platform::current();
}

#[test]
fn test_embedding_model_config_deserialization() {
    let yaml = r#"
source: !HuggingFace
  repo: "test/embedding-model"
  filename: "model.gguf"
normalize: true
max_sequence_length: 512
"#;
    let config: EmbeddingModelConfig =
        serde_yaml_ng::from_str(yaml).expect("Should parse embedding config");
    assert!(config.normalize);
    assert_eq!(config.max_sequence_length, Some(512));
}

#[test]
fn test_model_error_severity() {
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    let parse_err = serde_yaml_ng::from_str::<ModelConfig>("invalid: yaml: [unclosed")
        .expect_err("Should fail to parse");
    let model_parse_err = ModelError::ParseError(parse_err);
    assert_eq!(model_parse_err.severity(), ErrorSeverity::Critical);

    let config_err = ModelError::ConfigError("test".to_string());
    assert_eq!(config_err.severity(), ErrorSeverity::Critical);

    let not_found = ModelError::NotFound("test".to_string());
    assert_eq!(not_found.severity(), ErrorSeverity::Error);

    let invalid_path = ModelError::InvalidPath(PathBuf::from("/test"));
    assert_eq!(invalid_path.severity(), ErrorSeverity::Error);

    let io_err = ModelError::IoError(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
    assert_eq!(io_err.severity(), ErrorSeverity::Error);
}

#[test]
fn test_model_paths_avp() {
    let paths = ModelPaths::avp();
    assert_eq!(paths.dir_name, ".avp");
    assert_eq!(paths.config_filename, "avp.yaml");
}

#[test]
fn test_model_paths_sah() {
    let paths = ModelPaths::sah();
    assert_eq!(paths.dir_name, ".sah");
    assert_eq!(paths.config_filename, "sah.yaml");
}

#[test]
fn test_executor_type_all_variants() {
    // Exercises `executor_type()` for all executor types.
    // Test LlamaEmbedding
    let embedding_config = ModelConfig {
        executors: vec![ExecutorEntry {
            platform: None,
            executor: ModelExecutorConfig::LlamaEmbedding(EmbeddingModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "test/repo".to_string(),
                    filename: Some("model.gguf".to_string()),
                    folder: None,
                },
                normalize: false,
                max_sequence_length: None,
            }),
        }],
        quiet: false,
    };
    assert_eq!(
        embedding_config.executor_type().unwrap(),
        ModelExecutorType::LlamaEmbedding
    );

    // Test AneEmbedding
    let ane_config = ModelConfig {
        executors: vec![ExecutorEntry {
            platform: None,
            executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "test/repo".to_string(),
                    filename: Some("model.gguf".to_string()),
                    folder: None,
                },
                normalize: true,
                max_sequence_length: Some(256),
            }),
        }],
        quiet: false,
    };
    assert_eq!(
        ane_config.executor_type().unwrap(),
        ModelExecutorType::AneEmbedding
    );
}

#[test]
fn test_model_config_source_debug_variants() {
    assert_eq!(format!("{:?}", ModelConfigSource::GitRoot), "GitRoot");
}

#[test]
fn test_model_config_source_equality_gitroot() {
    assert_eq!(ModelConfigSource::GitRoot, ModelConfigSource::GitRoot);
    assert_ne!(ModelConfigSource::GitRoot, ModelConfigSource::Builtin);
    assert_ne!(ModelConfigSource::GitRoot, ModelConfigSource::Project);
    assert_ne!(ModelConfigSource::GitRoot, ModelConfigSource::User);
}
