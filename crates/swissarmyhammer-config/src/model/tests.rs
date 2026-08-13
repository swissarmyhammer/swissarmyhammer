//! Tests for the [model configuration](super).
//!
//! The tests are split by subject, one module for each. The review engine
//! renders a whole file into one agent prompt, and a file over the per-file
//! prompt cap is not reviewed at all, so a test tree this size has to be
//! several files rather than one.
//!
//! - [`serialization`] — the YAML and JSON forms of a model configuration,
//!   a model source, an agent source and an agent record.
//! - [`parsing`] — the description and the agent configuration a model file
//!   carries, in front matter and in comments.
//! - [`manager`] — `ModelManager`: loading a directory, the precedence of
//!   builtin, user and project models, and finding the configuration file.
//! - [`resolution`] — which executor a model resolves to, and how the
//!   platform narrows the choice.
//! - [`paths`] — the checks a configuration path and a directory must pass.
//! - [`loading`] — reading one model file, and what a directory walk keeps.
//! - [`config_structure`] — `ensure_config_structure` against each layout it
//!   can meet.
//! - [`types`] — the small types: the error severity, the platform, the
//!   executor kinds, the model paths and the configuration source.
//!
//! This module carries what those eight share: the imports and the minimal
//! embedding model configuration fixture.

mod config_structure;
mod loading;
mod manager;
mod parsing;
mod paths;
mod resolution;
mod serialization;
mod types;
use super::*;
use swissarmyhammer_common::test_utils::CurrentDirGuard;

/// A minimal `llama-embedding` model configuration, the shape every model
/// YAML now takes.
fn embedding_model_config() -> ModelConfig {
    ModelConfig {
        executors: vec![ExecutorEntry {
            platform: None,
            executor: ModelExecutorConfig::LlamaEmbedding(EmbeddingModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "test/embed".to_string(),
                    filename: Some("test.gguf".to_string()),
                    folder: None,
                },
                normalize: true,
                max_sequence_length: None,
            }),
        }],
        quiet: false,
    }
}
