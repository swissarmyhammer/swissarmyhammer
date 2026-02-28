//! Integration tests for model configuration wiring.
//!
//! These tests verify that:
//! 1. A non-default model config flows through AvpContext to validator execution
//! 2. Validators execute correctly regardless of which model config is plugged in
//! 3. The model config is preserved and accessible throughout the pipeline

mod test_helpers;

use agent_client_protocol_extras::PlaybackAgent;
use avp_common::{
    chain::ChainFactory,
    context::AvpContext,
    turn::TurnStateManager,
    validator::ValidatorLoader,
};
use std::fs;
use std::sync::Arc;
use swissarmyhammer_config::model::{LlamaAgentConfig, ModelConfig, ModelExecutorConfig};
use tempfile::TempDir;
use test_helpers::fixtures_dir;

// ============================================================================
// Helpers
// ============================================================================

/// Create an AvpContext with a PlaybackAgent and an explicit ModelConfig.
fn create_context_with_model(
    temp: &TempDir,
    fixture_name: &str,
    model_config: ModelConfig,
) -> AvpContext {
    let fixture_path = fixtures_dir().join(fixture_name);
    let agent = PlaybackAgent::new(fixture_path, "claude");
    let notification_rx = agent.subscribe_notifications();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let context =
        AvpContext::with_agent_and_model(Arc::new(agent), notification_rx, model_config)
            .expect("Should create context with playback agent and model config");

    std::env::set_current_dir(&original_dir).unwrap();
    context
}

// ============================================================================
// Model Config Plumbing Tests
// ============================================================================

/// Verify that a LlamaAgent model config is preserved through AvpContext.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_llama_model_config_preserved_in_context() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let llama_config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());
    let context =
        create_context_with_model(&temp, "no_secrets_clean_code.json", llama_config);

    assert!(
        matches!(
            context.model_config().executor,
            ModelExecutorConfig::LlamaAgent(_)
        ),
        "Context should preserve the LlamaAgent model config, got {:?}",
        context.model_config().executor
    );
}

/// Verify that a ClaudeCode model config is preserved through AvpContext.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_claude_model_config_preserved_in_context() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let claude_config = ModelConfig::claude_code();
    let context =
        create_context_with_model(&temp, "no_secrets_clean_code.json", claude_config);

    assert!(
        matches!(
            context.model_config().executor,
            ModelExecutorConfig::ClaudeCode(_)
        ),
        "Context should preserve the ClaudeCode model config"
    );
}

// ============================================================================
// Validator Execution with Plugged-in Model
// ============================================================================

/// Run a PostToolUse validator through the full chain with a LlamaAgent model config.
///
/// Uses a fixture where the no-secrets validator detects a secret and blocks.
/// Proves the end-to-end path: model config → context → chain → validator blocks.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_validator_blocks_with_llama_model_config() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let llama_config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());
    let context = create_context_with_model(
        &temp,
        "post_tool_use_no_secrets_fail.json",
        llama_config,
    );

    // Verify model config survived
    assert!(matches!(
        context.model_config().executor,
        ModelExecutorConfig::LlamaAgent(_)
    ));

    let turn_state = Arc::new(TurnStateManager::new(temp.path()));
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let factory = ChainFactory::new(Arc::new(context), Arc::new(loader), turn_state);
    let mut chain = factory.post_tool_use_chain();

    let input: avp_common::types::PostToolUseInput = serde_json::from_value(serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": {
            "file_path": "config.ts",
            "content": "const apiKey = 'sk-proj-1234567890';"
        },
        "tool_response": {
            "filePath": "config.ts",
            "success": true
        },
        "tool_use_id": "toolu_test123"
    }))
    .unwrap();

    let (chain_output, _) = chain.execute(&input).await.unwrap();

    // The no-secrets validator should have blocked
    assert!(
        chain_output.validator_block.is_some(),
        "Validator should block with LlamaAgent model config — the model config \
         should not interfere with validator execution. Got: {:?}",
        chain_output
    );
}

/// Run a PostToolUse chain with a non-matching tool using a LlamaAgent model config.
///
/// Read operations don't match any PostToolUse validators, so the chain should
/// pass through cleanly without needing agent interaction.
/// Proves: model config doesn't break the non-matching path.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_chain_passes_with_llama_model_config_nonmatching_tool() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let llama_config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());
    let context = create_context_with_model(
        &temp,
        "no_secrets_clean_code.json",
        llama_config,
    );

    assert!(matches!(
        context.model_config().executor,
        ModelExecutorConfig::LlamaAgent(_)
    ));

    let turn_state = Arc::new(TurnStateManager::new(temp.path()));
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let factory = ChainFactory::new(Arc::new(context), Arc::new(loader), turn_state);
    let mut chain = factory.post_tool_use_chain();

    // Read tool doesn't match PostToolUse validators — no agent call needed
    let input: avp_common::types::PostToolUseInput = serde_json::from_value(serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Read",
        "tool_input": {
            "file_path": "app.rs"
        },
        "tool_response": {
            "filePath": "app.rs",
            "content": "fn main() {}"
        },
        "tool_use_id": "toolu_test789"
    }))
    .unwrap();

    let (chain_output, _) = chain.execute(&input).await.unwrap();

    assert!(
        chain_output.validator_block.is_none(),
        "Read tool should not trigger any validator blocks. Got: {:?}",
        chain_output.validator_block
    );
    assert!(
        chain_output.continue_execution,
        "Chain should continue execution for non-matching tool"
    );
}

/// Verify the agent is accessible through context with a non-default model config.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_agent_accessible_with_llama_model_config() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let llama_config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());
    let context =
        create_context_with_model(&temp, "no_secrets_clean_code.json", llama_config);

    let result = context.agent().await;
    assert!(
        result.is_ok(),
        "Should be able to get agent with LlamaAgent model config"
    );
}
