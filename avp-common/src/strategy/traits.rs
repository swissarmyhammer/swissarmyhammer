//! Strategy trait for agent-specific hook processing.

use async_trait::async_trait;

use crate::chain::HookInputType;
use crate::error::AvpError;
use crate::types::HookOutput;

/// Strategy for processing hooks with typed Input -> Output transformation.
///
/// Each strategy implementation defines how to process a specific hook type's
/// input and produce the corresponding output. The chain of responsibility
/// operates on the typed input.
///
/// # Type Parameters
/// - `I`: The typed input (e.g., `PreToolUseInput`)
#[async_trait]
pub trait TypedHookStrategy<I: HookInputType>: Send + Sync {
    /// Process the typed input and return an output with exit code.
    ///
    /// Exit codes:
    /// - 0: Success
    /// - 2: Blocking error
    async fn process(&self, input: I) -> Result<(HookOutput, i32), AvpError>;

    /// Get the name of this strategy for debugging.
    fn name(&self) -> &'static str;
}

/// Strategy for processing raw JSON from an agent platform.
///
/// This trait handles the parsing/dispatch layer. Implementations know how to:
/// - Detect if they can handle a given JSON input
/// - Parse the JSON into the correct typed input
/// - Dispatch to the appropriate `TypedHookStrategy`
/// - Return the output formatted for the platform
///
/// # Example
/// ```ignore
/// struct ClaudeCodeHookStrategy { ... }
///
/// #[async_trait]
/// impl AgentHookStrategy for ClaudeCodeHookStrategy {
///     fn name(&self) -> &'static str { "ClaudeCode" }
///
///     fn can_handle(&self, input: &Value) -> bool {
///         // Check for Claude Code hook structure
///         input.get("hook_event_name").is_some()
///     }
///
///     async fn process(&self, input: Value) -> Result<(HookOutput, i32), AvpError> {
///         // Parse hook_event_name, dispatch to typed handler
///     }
/// }
/// ```
#[async_trait]
pub trait AgentHookStrategy: Send + Sync {
    /// The name of this agent platform (e.g., "ClaudeCode").
    fn name(&self) -> &'static str;

    /// Check if this strategy can handle the given input.
    fn can_handle(&self, input: &serde_json::Value) -> bool;

    /// Process the raw JSON input and return output with exit code.
    async fn process(&self, input: serde_json::Value) -> Result<(HookOutput, i32), AvpError>;
}
