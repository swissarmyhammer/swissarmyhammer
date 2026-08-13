//! Which executor a model resolves to.
//!
//! The old single-executor form and the new executors list, and how the
//! platform narrows the choice before the universal entry is taken.

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
  type: llama-embedding
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
        ModelExecutorType::LlamaEmbedding
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
      type: llama-embedding
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

#[test]
fn test_platform_selection_prefers_platform_match() {
    let config = ModelConfig {
        executors: vec![
            ExecutorEntry {
                platform: Some(Platform::current()),
                executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "test/ane".to_string(),
                        filename: None,
                        folder: None,
                    },
                    normalize: true,
                    max_sequence_length: None,
                }),
            },
            ExecutorEntry {
                platform: None,
                executor: ModelExecutorConfig::LlamaEmbedding(EmbeddingModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "test/llama".to_string(),
                        filename: None,
                        folder: None,
                    },
                    normalize: true,
                    max_sequence_length: None,
                }),
            },
        ],
        quiet: false,
    };
    // First entry matches current platform, so it should be selected
    assert_eq!(
        config.executor_type().unwrap(),
        ModelExecutorType::AneEmbedding
    );
}

#[test]
fn test_platform_selection_falls_back_to_universal() {
    // Use a platform that doesn't match current
    let non_matching_platform = if Platform::current() == Platform::MacosArm64 {
        Platform::LinuxX86_64
    } else {
        Platform::MacosArm64
    };

    let config = ModelConfig {
        executors: vec![
            ExecutorEntry {
                platform: Some(non_matching_platform),
                executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "test/ane".to_string(),
                        filename: None,
                        folder: None,
                    },
                    normalize: true,
                    max_sequence_length: None,
                }),
            },
            ExecutorEntry {
                platform: None,
                executor: ModelExecutorConfig::LlamaEmbedding(EmbeddingModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "test/llama".to_string(),
                        filename: None,
                        folder: None,
                    },
                    normalize: true,
                    max_sequence_length: None,
                }),
            },
        ],
        quiet: false,
    };
    // First entry doesn't match, second is universal fallback
    assert_eq!(
        config.executor_type().unwrap(),
        ModelExecutorType::LlamaEmbedding
    );
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

#[test]
fn test_platform_current_is_stable() {
    assert_eq!(Platform::current(), Platform::current());
}
