//! Validator execution via ACP agent.
//!
//! This module provides the `ValidatorRunner` which executes validators against
//! hook events by calling an LLM agent via the Agent Client Protocol (ACP).

use std::sync::Arc;
use swissarmyhammer_agent::{self as acp, AcpAgentHandle, AgentResponse, McpServerConfig};
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
use tokio::sync::Mutex;

use crate::error::AvpError;
use crate::types::HookType;
use crate::validator::{
    create_executed_validator, parse_validator_response, render_validator_prompt, ExecutedValidator,
    Validator, VALIDATOR_PROMPT_NAME,
};

/// Configuration for the validator runner's ACP agent.
#[derive(Clone, Default)]
pub struct ValidatorAgentConfig {
    /// Model configuration for the ACP agent
    pub model_config: ModelConfig,
    /// Optional MCP server configuration
    pub mcp_config: Option<McpServerConfig>,
}

/// Executes validators via ACP agent calls.
///
/// The `ValidatorRunner` handles:
/// 1. Rendering validator prompts using the `.validator` template
/// 2. Executing prompts via ACP agent (Claude or Llama)
/// 3. Parsing LLM responses into pass/fail results
/// 4. Creating `ExecutedValidator` results with metadata
pub struct ValidatorRunner {
    /// Agent configuration
    agent_config: ValidatorAgentConfig,
    /// Prompt library containing the .validator prompt
    prompt_library: Arc<PromptLibrary>,
    /// Cached ACP agent handle (created lazily)
    agent_handle: Arc<Mutex<Option<AcpAgentHandle>>>,
}

impl ValidatorRunner {
    /// Create a new ValidatorRunner with the given agent configuration.
    ///
    /// Loads the PromptLibrary containing the `.validator` prompt.
    pub fn new(agent_config: ValidatorAgentConfig) -> Result<Self, AvpError> {
        // Load all prompts including the builtin .validator prompt
        let mut prompt_library = PromptLibrary::new();
        let mut resolver = PromptResolver::new();
        resolver
            .load_all_prompts(&mut prompt_library)
            .map_err(|e| AvpError::Agent(format!("Failed to load prompt library: {}", e)))?;

        // Verify .validator prompt exists
        prompt_library.get(VALIDATOR_PROMPT_NAME).map_err(|e| {
            AvpError::Agent(format!(
                ".validator prompt not found in prompt library: {}",
                e
            ))
        })?;

        tracing::debug!(".validator prompt loaded successfully");

        Ok(Self {
            agent_config,
            prompt_library: Arc::new(prompt_library),
            agent_handle: Arc::new(Mutex::new(None)),
        })
    }

    /// Create a ValidatorRunner with default configuration.
    ///
    /// Uses the default model configuration (Claude Code).
    pub fn with_defaults() -> Result<Self, AvpError> {
        Self::new(ValidatorAgentConfig::default())
    }

    /// Create a ValidatorRunner with a pre-created agent handle.
    ///
    /// This is useful for testing with PlaybackAgent or other mock agents.
    pub fn with_agent_handle(agent_handle: AcpAgentHandle) -> Result<Self, AvpError> {
        // Load all prompts including the builtin .validator prompt
        let mut prompt_library = PromptLibrary::new();
        let mut resolver = PromptResolver::new();
        resolver
            .load_all_prompts(&mut prompt_library)
            .map_err(|e| AvpError::Agent(format!("Failed to load prompt library: {}", e)))?;

        // Verify .validator prompt exists
        prompt_library.get(VALIDATOR_PROMPT_NAME).map_err(|e| {
            AvpError::Agent(format!(
                ".validator prompt not found in prompt library: {}",
                e
            ))
        })?;

        tracing::debug!(".validator prompt loaded successfully (with injected agent)");

        Ok(Self {
            agent_config: ValidatorAgentConfig::default(),
            prompt_library: Arc::new(prompt_library),
            agent_handle: Arc::new(Mutex::new(Some(agent_handle))),
        })
    }

    /// Get or create the ACP agent (lazy initialization).
    async fn get_or_create_agent(&self) -> Result<(), AvpError> {
        let mut guard = self.agent_handle.lock().await;
        if guard.is_none() {
            tracing::debug!("Creating ACP agent for validator execution...");
            let start = std::time::Instant::now();
            let agent = acp::create_agent(
                &self.agent_config.model_config,
                self.agent_config.mcp_config.clone(),
            )
            .await
            .map_err(|e| AvpError::Agent(format!("Failed to create agent: {}", e)))?;
            tracing::debug!(
                "ACP agent created in {:.2}s",
                start.elapsed().as_secs_f64()
            );
            *guard = Some(agent);
        }
        Ok(())
    }

    /// Create a session-scoped agent handle for execution.
    async fn create_session_handle(&self) -> Result<AcpAgentHandle, AvpError> {
        let guard = self.agent_handle.lock().await;
        let main_handle = guard
            .as_ref()
            .ok_or_else(|| AvpError::Agent("Agent not initialized".to_string()))?;

        Ok(AcpAgentHandle {
            agent: Arc::clone(&main_handle.agent),
            notification_rx: main_handle.notification_rx.resubscribe(),
        })
    }

    /// Execute a single validator against a hook event context.
    ///
    /// Returns an `ExecutedValidator` with the result and validator metadata.
    pub async fn execute_validator(
        &self,
        validator: &Validator,
        hook_type: HookType,
        context: &serde_json::Value,
    ) -> ExecutedValidator {
        // Render the validator prompt
        let prompt_result =
            render_validator_prompt(&self.prompt_library, validator, hook_type, context);

        let prompt_text = match prompt_result {
            Ok(text) => text,
            Err(e) => {
                tracing::error!(
                    "Failed to render validator '{}' prompt: {}",
                    validator.name(),
                    e
                );
                return create_executed_validator(
                    validator,
                    crate::validator::ValidatorResult::fail(format!(
                        "Failed to render prompt: {}",
                        e
                    )),
                );
            }
        };

        // Ensure agent is created
        if let Err(e) = self.get_or_create_agent().await {
            tracing::error!("Failed to create agent for validator '{}': {}", validator.name(), e);
            return create_executed_validator(
                validator,
                crate::validator::ValidatorResult::fail(format!("Agent creation failed: {}", e)),
            );
        }

        // Create session handle and execute
        let session_handle = match self.create_session_handle().await {
            Ok(h) => h,
            Err(e) => {
                tracing::error!(
                    "Failed to create session for validator '{}': {}",
                    validator.name(),
                    e
                );
                return create_executed_validator(
                    validator,
                    crate::validator::ValidatorResult::fail(format!(
                        "Session creation failed: {}",
                        e
                    )),
                );
            }
        };

        let mut session_handle = session_handle;

        // Execute via ACP - no custom mode, the .validator template has everything needed
        let response: Result<AgentResponse, _> = acp::execute_prompt(
            &mut session_handle,
            None, // No system prompt - the .validator template has everything
            None, // No custom mode
            prompt_text,
        )
        .await;

        match response {
            Ok(agent_response) => {
                let result = parse_validator_response(&agent_response.content);
                tracing::debug!(
                    "Validator '{}' result: {} - {}",
                    validator.name(),
                    if result.passed() { "PASSED" } else { "FAILED" },
                    result.message()
                );
                create_executed_validator(validator, result)
            }
            Err(e) => {
                tracing::error!(
                    "Agent execution failed for validator '{}': {}",
                    validator.name(),
                    e
                );
                create_executed_validator(
                    validator,
                    crate::validator::ValidatorResult::fail(format!(
                        "Agent execution failed: {}",
                        e
                    )),
                )
            }
        }
    }

    /// Execute multiple validators against a hook event context.
    ///
    /// Executes validators sequentially (to avoid overwhelming the agent).
    /// Returns a list of `ExecutedValidator` results.
    pub async fn execute_validators(
        &self,
        validators: &[&Validator],
        hook_type: HookType,
        context: &serde_json::Value,
    ) -> Vec<ExecutedValidator> {
        let mut results = Vec::with_capacity(validators.len());

        for validator in validators {
            let result = self.execute_validator(validator, hook_type, context).await;
            results.push(result);

            // If this validator blocked (failed + error severity), stop early
            if results.last().map(|r| r.is_blocking()).unwrap_or(false) {
                tracing::info!(
                    "Validator '{}' blocked - stopping further validation",
                    validator.name()
                );
                break;
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_agent_config_default() {
        let config = ValidatorAgentConfig::default();
        assert!(config.mcp_config.is_none());
    }

    // Note: Full integration tests require an ACP agent to be available.
    // These are tested through integration tests or manual testing.
}
