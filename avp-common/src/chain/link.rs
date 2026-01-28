//! Chain link trait and result types.

use std::marker::PhantomData;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::ChainError;

use super::context::ChainContext;
use super::output::LinkOutput;
use super::VALIDATOR_BLOCK_EXIT_CODE;

/// Marker trait for hook input types.
///
/// All hook-specific input types implement this trait, enabling
/// generic chain links that only accept valid hook inputs.
pub trait HookInputType: Clone + Send + Sync + Serialize + DeserializeOwned + 'static {}

/// Result of processing a chain link.
#[derive(Debug)]
pub enum ChainResult {
    /// Continue to the next link, optionally with partial output.
    Continue(Option<LinkOutput>),

    /// Stop chain processing with the given output.
    Stop(LinkOutput),

    /// An error occurred during processing.
    Error(ChainError),
}

impl ChainResult {
    /// Create a continue result with no output.
    pub fn continue_empty() -> Self {
        ChainResult::Continue(None)
    }

    /// Create a continue result with output.
    pub fn continue_with(output: LinkOutput) -> Self {
        ChainResult::Continue(Some(output))
    }

    /// Create a stop result.
    pub fn stop(output: LinkOutput) -> Self {
        ChainResult::Stop(output)
    }

    /// Create an error result.
    pub fn error(link: impl Into<String>, reason: impl Into<String>) -> Self {
        ChainResult::Error(ChainError::LinkFailed {
            link: link.into(),
            reason: reason.into(),
        })
    }
}

/// A link in the processing chain, generic over the input type.
///
/// Chain links process typed input data and can modify the chain context.
/// They return a `ChainResult` indicating whether to continue, stop, or error.
///
/// Links are async to support operations like validator execution that
/// require async I/O.
#[async_trait(?Send)]
pub trait ChainLink<I: HookInputType>: Send + Sync {
    /// Process the input and potentially modify the context.
    ///
    /// # Arguments
    /// * `input` - The typed hook input to process
    /// * `ctx` - Mutable context for sharing state between links
    ///
    /// # Returns
    /// A `ChainResult` indicating the outcome of processing
    async fn process(&self, input: &I, ctx: &mut ChainContext) -> ChainResult;

    /// Get the human-readable name of this link for logging/debugging.
    fn name(&self) -> &'static str;

    /// Whether this link can short-circuit the chain.
    ///
    /// If true, this link may return `ChainResult::Stop` to halt processing.
    fn can_short_circuit(&self) -> bool {
        false
    }
}

/// A simple pass-through link that does nothing.
#[derive(Debug, Default)]
pub struct PassThroughLink<I: HookInputType> {
    _phantom: PhantomData<I>,
}

impl<I: HookInputType> PassThroughLink<I> {
    /// Create a new pass-through link.
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

#[async_trait(?Send)]
impl<I: HookInputType> ChainLink<I> for PassThroughLink<I> {
    async fn process(&self, _input: &I, _ctx: &mut ChainContext) -> ChainResult {
        ChainResult::continue_empty()
    }

    fn name(&self) -> &'static str {
        "PassThrough"
    }
}

/// A link that validates a condition on the input.
pub struct ValidationLink<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> Result<(), String> + Send + Sync,
{
    name: &'static str,
    validator: F,
    _phantom: PhantomData<I>,
}

impl<I, F> ValidationLink<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> Result<(), String> + Send + Sync,
{
    /// Create a new validation link.
    pub fn new(name: &'static str, validator: F) -> Self {
        Self {
            name,
            validator,
            _phantom: PhantomData,
        }
    }
}

#[async_trait(?Send)]
impl<I, F> ChainLink<I> for ValidationLink<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> Result<(), String> + Send + Sync,
{
    async fn process(&self, input: &I, ctx: &mut ChainContext) -> ChainResult {
        match (self.validator)(input) {
            Ok(()) => ChainResult::continue_empty(),
            Err(reason) => {
                ctx.set_exit_code(VALIDATOR_BLOCK_EXIT_CODE);
                ChainResult::stop(LinkOutput::block(reason))
            }
        }
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn can_short_circuit(&self) -> bool {
        true
    }
}

/// A link that transforms input and adds context.
pub struct ContextLink<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> Option<String> + Send + Sync,
{
    name: &'static str,
    context_fn: F,
    _phantom: PhantomData<I>,
}

impl<I, F> ContextLink<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> Option<String> + Send + Sync,
{
    /// Create a new context link.
    pub fn new(name: &'static str, context_fn: F) -> Self {
        Self {
            name,
            context_fn,
            _phantom: PhantomData,
        }
    }
}

#[async_trait(?Send)]
impl<I, F> ChainLink<I> for ContextLink<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> Option<String> + Send + Sync,
{
    async fn process(&self, input: &I, _ctx: &mut ChainContext) -> ChainResult {
        if let Some(message) = (self.context_fn)(input) {
            ChainResult::continue_with(LinkOutput::empty().with_message(message))
        } else {
            ChainResult::continue_empty()
        }
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PreToolUseInput;

    #[tokio::test]
    async fn test_pass_through_link() {
        let link: PassThroughLink<PreToolUseInput> = PassThroughLink::new();
        let input: PreToolUseInput = serde_json::from_value(serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .unwrap();
        let mut ctx = ChainContext::new();

        match link.process(&input, &mut ctx).await {
            ChainResult::Continue(None) => {}
            _ => panic!("Expected Continue(None)"),
        }
    }

    #[tokio::test]
    async fn test_validation_link_pass() {
        let link = ValidationLink::new("TestValidator", |input: &PreToolUseInput| {
            if input.tool_name == "Bash" {
                Ok(())
            } else {
                Err("Only Bash allowed".to_string())
            }
        });

        let input: PreToolUseInput = serde_json::from_value(serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .unwrap();
        let mut ctx = ChainContext::new();

        match link.process(&input, &mut ctx).await {
            ChainResult::Continue(_) => {}
            _ => panic!("Expected Continue"),
        }
    }

    #[tokio::test]
    async fn test_validation_link_fail() {
        let link = ValidationLink::new("TestValidator", |input: &PreToolUseInput| {
            if input.tool_name == "Bash" {
                Ok(())
            } else {
                Err("Only Bash allowed".to_string())
            }
        });

        let input: PreToolUseInput = serde_json::from_value(serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": "/test"}
        }))
        .unwrap();
        let mut ctx = ChainContext::new();

        match link.process(&input, &mut ctx).await {
            ChainResult::Stop(_) => {}
            _ => panic!("Expected Stop"),
        }
    }
}
