//! Logging utilities for SwissArmyHammer
//!
//! This module provides utilities for formatting and displaying log messages.

use serde::Serialize;
use std::fmt::Debug;

/// Wrapper for pretty-printing types in logs as YAML
///
/// Use this in tracing statements to automatically format complex types
/// as YAML with a newline before the content:
///
/// ```ignore
/// use swissarmyhammer_common::Pretty;
/// use tracing::info;
///
/// let config = MyConfig { /* ... */ };
/// info!("Config: {}", Pretty(&config));
/// ```
///
/// Outputs YAML format with a leading newline. Types must implement Serialize + Debug.
/// Debug is used as a fallback if YAML serialization fails.
pub struct Pretty<T>(pub T);

impl<T: Serialize + Debug> std::fmt::Display for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_yaml::to_string(&self.0) {
            Ok(yaml) => write!(f, "\n{}", yaml),
            Err(_) => write!(f, "\n{:#?}", self.0),
        }
    }
}

impl<T: Serialize + Debug> std::fmt::Debug for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_yaml::to_string(&self.0) {
            Ok(yaml) => write!(f, "\n{}", yaml),
            Err(_) => write!(f, "\n{:#?}", self.0),
        }
    }
}

/// Default maximum length for truncated log messages
const DEFAULT_TRUNCATE_LENGTH: usize = 500;

/// Truncate a string for logging, adding "..." if truncated
fn truncate_for_log(text: &str, max_len: usize) -> String {
    if text.len() > max_len {
        format!("{}...", &text[..max_len])
    } else {
        text.to_string()
    }
}

/// Log a prompt being sent to an agent
///
/// Use this to log prompts consistently across all agent implementations.
///
/// # Arguments
/// * `agent_name` - Name of the agent (e.g., "Claude", "Llama")
/// * `prompt` - The prompt text being sent
///
/// # Example
/// ```ignore
/// use swissarmyhammer_common::logging::log_prompt;
/// log_prompt("Claude", &prompt_text);
/// ```
pub fn log_prompt(agent_name: &str, prompt: &str) {
    tracing::info!(
        "📤 {} PROMPT ({} chars): {}",
        agent_name,
        prompt.len(),
        truncate_for_log(prompt, DEFAULT_TRUNCATE_LENGTH)
    );
}

/// Log a response received from an agent
///
/// Use this to log responses consistently across all agent implementations.
///
/// # Arguments
/// * `agent_name` - Name of the agent (e.g., "Claude", "Llama")
/// * `response` - The response text received
///
/// # Example
/// ```ignore
/// use swissarmyhammer_common::logging::log_response;
/// log_response("Claude", &response_text);
/// ```
pub fn log_response(agent_name: &str, response: &str) {
    tracing::info!(
        "📥 {} RESPONSE ({} chars): {}",
        agent_name,
        response.len(),
        truncate_for_log(response, DEFAULT_TRUNCATE_LENGTH)
    );
}

/// Log generated content from an agent (alias for log_response with different emoji)
///
/// Use this when logging the final generated content after processing.
///
/// # Arguments
/// * `agent_name` - Name of the agent (e.g., "Claude", "Llama")
/// * `content` - The generated content
pub fn log_generated_content(agent_name: &str, content: &str) {
    tracing::info!(
        "✨ {} GENERATED ({} chars): {}",
        agent_name,
        content.len(),
        truncate_for_log(content, DEFAULT_TRUNCATE_LENGTH)
    );
}
