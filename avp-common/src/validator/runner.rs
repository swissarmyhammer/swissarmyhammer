//! Validator execution via ACP Agent.
//!
//! This module provides the `ValidatorRunner` which executes validators against
//! hook events by calling an Agent via the Agent Client Protocol (ACP).
//!
//! The agent is obtained from `AvpContext`, which handles lazy creation of
//! ClaudeAgent in production or injection of PlaybackAgent for testing.

use std::sync::Arc;

use agent_client_protocol::{Agent, SessionNotification};
use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};
use tokio::sync::broadcast;

use crate::error::AvpError;
use crate::types::HookType;
use crate::validator::{
    create_executed_validator, parse_validator_response, render_validator_prompt,
    ExecutedValidator, Validator, VALIDATOR_PROMPT_NAME,
};

/// Executes validators via ACP Agent calls.
///
/// The `ValidatorRunner` handles:
/// 1. Rendering validator prompts using the `.validator` template
/// 2. Executing prompts via the provided Agent
/// 3. Parsing LLM responses into pass/fail results
/// 4. Creating `ExecutedValidator` results with metadata
///
/// # Usage
///
/// Get the agent from `AvpContext` and create a runner:
/// ```ignore
/// let (agent, notifications) = context.agent().await?;
/// let runner = ValidatorRunner::new(agent, notifications)?;
/// ```
pub struct ValidatorRunner {
    /// Prompt library containing the .validator prompt
    prompt_library: Arc<PromptLibrary>,
    /// Agent for executing prompts
    agent: Arc<dyn Agent + Send + Sync>,
    /// Notification sender for resubscription
    notifications: broadcast::Sender<SessionNotification>,
}

impl ValidatorRunner {
    /// Create a new ValidatorRunner with the given agent.
    ///
    /// The agent and notifications should be obtained from `AvpContext::agent()`.
    pub fn new(
        agent: Arc<dyn Agent + Send + Sync>,
        notifications: broadcast::Receiver<SessionNotification>,
    ) -> Result<Self, AvpError> {
        let prompt_library = Self::load_prompt_library()?;

        // Create a sender so we can resubscribe for each validator execution
        let (tx, _) = broadcast::channel(256);
        let tx_clone = tx.clone();

        // Forward notifications from the provided receiver to our sender
        tokio::spawn(async move {
            let mut rx = notifications;
            while let Ok(notification) = rx.recv().await {
                let _ = tx_clone.send(notification);
            }
        });

        Ok(Self {
            prompt_library: Arc::new(prompt_library),
            agent,
            notifications: tx,
        })
    }

    /// Load and validate the prompt library.
    fn load_prompt_library() -> Result<PromptLibrary, AvpError> {
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
        Ok(prompt_library)
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

        // Get a fresh notification receiver for this execution
        let notifications = self.notifications.subscribe();

        // Execute via claude_agent::execute_prompt_with_agent helper
        let response =
            claude_agent::execute_prompt_with_agent(&*self.agent, notifications, prompt_text).await;

        match response {
            Ok(prompt_response) => {
                let result = parse_validator_response(&prompt_response.content);
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
    // Note: ValidatorRunner now requires an Agent.
    // Unit tests are in integration tests with PlaybackAgent via AvpContext.
}
