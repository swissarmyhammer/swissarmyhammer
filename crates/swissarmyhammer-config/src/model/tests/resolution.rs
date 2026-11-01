//! Which executor a model resolves to.
//!
//! The old single-executor form and the new executors list, and which entry
//! the selection takes.

use super::*;

// Model Resolution Tests
mod model_resolution_tests {
    use super::*;

    fn setup_test_env() -> tempfile::TempDir {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        temp_dir
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_model_config_format() {
        let temp_dir = setup_test_env();
        let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();

        let config_path = ModelManager::ensure_config_structure(&ModelPaths::sah()).unwrap();
        std::fs::write(&config_path, "model: sonnet\n").unwrap();

        assert_eq!(
            ModelManager::get_chat_model(&ModelPaths::sah())
                .unwrap()
                .unwrap(),
            "sonnet"
        );
    }

    // ====================================================================
    // Review-specific model target tests
    // ====================================================================
}

// ========================================================================
// Multi-executor and platform selection tests
// ========================================================================

#[test]
fn test_parse_old_executor_format_backward_compat() {
    let yaml = r#"
executor:
  type: ane-embedding
  config:
    source: !HuggingFace
      repo: "Qwen/Qwen3-Embedding-0.6B-GGUF"
      filename: "Qwen3-Embedding-0.6B-Q8_0.gguf"
    normalize: true
    max_sequence_length: 512
quiet: false
"#;
    let config: ModelConfig = serde_yaml_ng::from_str(yaml).expect("old format should parse");
    assert_eq!(config.executors.len(), 1);
    assert!(config.executors[0].platform.is_none());
    assert_eq!(
        config.executor_type().unwrap(),
        ModelExecutorType::AneEmbedding
    );
}

#[test]
fn test_parse_new_executors_list_format() {
    let yaml = r#"
executors:
  - platform: macos-arm64
    executor:
      type: ane-embedding
      config:
        source: !HuggingFace
          repo: "wballard/Qwen3-Embedding-0.6B-CoreML"
        normalize: true
  - executor:
      type: ane-embedding
      config:
        source: !HuggingFace
          repo: "Qwen/Qwen3-Embedding-0.6B-GGUF"
          filename: "Qwen3-Embedding-0.6B-Q8_0.gguf"
        normalize: true
        max_sequence_length: 512
quiet: false
"#;
    let config: ModelConfig = serde_yaml_ng::from_str(yaml).expect("new format should parse");
    assert_eq!(config.executors.len(), 2);
    assert_eq!(config.executors[0].platform, Some(Platform::MacosArm64));
    assert!(config.executors[1].platform.is_none());
}

/// The HuggingFace repo of the executor `select_executor` picks, so a test can
/// tell which of several same-type entries won.
fn selected_repo(config: &ModelConfig) -> String {
    match config.select_executor() {
        Some(ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
            source: ModelSource::HuggingFace { repo, .. },
            ..
        })) => repo.clone(),
        other => panic!("expected a HuggingFace ANE executor, got {other:?}"),
    }
}

#[test]
fn test_selection_takes_the_first_executor() {
    let entry = |repo: &str| ExecutorEntry {
        platform: None,
        executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
            source: ModelSource::HuggingFace {
                repo: repo.to_string(),
                filename: None,
                folder: None,
            },
            normalize: true,
            max_sequence_length: None,
        }),
    };
    let config = ModelConfig {
        executors: vec![entry("test/first"), entry("test/second")],
        quiet: false,
    };
    assert_eq!(selected_repo(&config), "test/first");
}

#[test]
fn test_ane_embedding_round_trip() {
    let config = ModelConfig {
        executors: vec![ExecutorEntry {
            platform: Some(Platform::MacosArm64),
            executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "wballard/test".to_string(),
                    filename: None,
                    folder: None,
                },
                normalize: true,
                max_sequence_length: Some(512),
            }),
        }],
        quiet: false,
    };

    let yaml = serde_yaml_ng::to_string(&config).expect("serialize");
    assert!(yaml.contains("ane-embedding"));
    assert!(yaml.contains("macos-arm64"));

    let deserialized: ModelConfig = serde_yaml_ng::from_str(&yaml).expect("deserialize");
    assert_eq!(deserialized.executors.len(), 1);
    assert_eq!(
        deserialized.executors[0].platform,
        Some(Platform::MacosArm64)
    );
}
