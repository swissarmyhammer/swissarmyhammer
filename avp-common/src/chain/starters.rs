//! Chain starters that determine initial chain behavior.
//! Starters run before any chain links and control whether the chain proceeds.

use std::marker::PhantomData;

use crate::error::ChainError;

use super::context::ChainContext;
use super::link::HookInputType;
use super::output::ChainOutput;
use super::VALIDATOR_BLOCK_EXIT_CODE;

/// Result from a chain starter.
#[derive(Debug)]
pub enum StarterResult {
    /// Continue with chain processing.
    Continue,

    /// Stop immediately with this output.
    Stop(ChainOutput),
}

/// A starter determines the initial behavior of a chain.
///
/// Starters run before any chain links and can either allow the chain
/// to proceed or immediately return an output.
pub trait ChainStarter<I: HookInputType>: Send + Sync {
    /// Start the chain processing.
    ///
    /// # Arguments
    /// * `input` - The typed hook input
    /// * `ctx` - Mutable context for setting initial state
    ///
    /// # Returns
    /// `StarterResult::Continue` to proceed with chain links, or
    /// `StarterResult::Stop` to immediately return an output
    fn start(&self, input: &I, ctx: &mut ChainContext) -> Result<StarterResult, ChainError>;

    /// Get the exit code this starter produces on success.
    fn exit_code(&self) -> i32;

    /// Get the name of this starter for debugging.
    fn name(&self) -> &'static str;
}

/// A starter that always continues with exit code 0.
///
/// Use this starter for chains that should process normally and
/// return success on completion.
#[derive(Debug, Default, Clone)]
pub struct SuccessStarter<I: HookInputType> {
    _phantom: PhantomData<I>,
}

impl<I: HookInputType> SuccessStarter<I> {
    /// Create a new success starter.
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<I: HookInputType> ChainStarter<I> for SuccessStarter<I> {
    fn start(&self, _input: &I, ctx: &mut ChainContext) -> Result<StarterResult, ChainError> {
        ctx.set_exit_code(0);
        Ok(StarterResult::Continue)
    }

    fn exit_code(&self) -> i32 {
        0
    }

    fn name(&self) -> &'static str {
        "SuccessStarter"
    }
}

/// A starter that immediately blocks with exit code 2.
///
/// Use this starter for chains that should fail immediately without
/// processing any links.
#[derive(Debug, Clone)]
pub struct BlockingErrorStarter<I: HookInputType> {
    /// The reason for blocking.
    reason: String,
    _phantom: PhantomData<I>,
}

impl<I: HookInputType> BlockingErrorStarter<I> {
    /// Create a new blocking error starter with the given reason.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            _phantom: PhantomData,
        }
    }
}

impl<I: HookInputType> ChainStarter<I> for BlockingErrorStarter<I> {
    fn start(&self, _input: &I, ctx: &mut ChainContext) -> Result<StarterResult, ChainError> {
        ctx.set_exit_code(VALIDATOR_BLOCK_EXIT_CODE);
        Ok(StarterResult::Stop(ChainOutput {
            continue_execution: false,
            stop_reason: Some(self.reason.clone()),
            ..Default::default()
        }))
    }

    fn exit_code(&self) -> i32 {
        VALIDATOR_BLOCK_EXIT_CODE
    }

    fn name(&self) -> &'static str {
        "BlockingErrorStarter"
    }
}

/// A conditional starter that runs a predicate to determine behavior.
///
/// If the predicate returns true, continues with the success exit code.
/// If false, blocks with the provided reason and error exit code.
#[derive(Clone)]
pub struct ConditionalStarter<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> bool + Send + Sync + Clone,
{
    /// The predicate function.
    predicate: F,

    /// Reason to use if predicate fails.
    block_reason: String,

    _phantom: PhantomData<I>,
}

impl<I, F> ConditionalStarter<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> bool + Send + Sync + Clone,
{
    /// Create a new conditional starter.
    pub fn new(predicate: F, block_reason: impl Into<String>) -> Self {
        Self {
            predicate,
            block_reason: block_reason.into(),
            _phantom: PhantomData,
        }
    }
}

impl<I, F> ChainStarter<I> for ConditionalStarter<I, F>
where
    I: HookInputType,
    F: Fn(&I) -> bool + Send + Sync + Clone,
{
    fn start(&self, input: &I, ctx: &mut ChainContext) -> Result<StarterResult, ChainError> {
        if (self.predicate)(input) {
            ctx.set_exit_code(0);
            Ok(StarterResult::Continue)
        } else {
            ctx.set_exit_code(VALIDATOR_BLOCK_EXIT_CODE);
            Ok(StarterResult::Stop(ChainOutput {
                continue_execution: false,
                stop_reason: Some(self.block_reason.clone()),
                ..Default::default()
            }))
        }
    }

    fn exit_code(&self) -> i32 {
        0 // Default exit code if predicate passes
    }

    fn name(&self) -> &'static str {
        "ConditionalStarter"
    }
}

/// A starter that checks for validator context and short-circuits if detected.
///
/// When AVP spawns validator agents, those agents have CLAUDE_ACP=1 set.
/// This prevents infinite recursion where validator agents' hooks trigger more validators.
///
/// **IMPORTANT: ALL hooks are skipped when in validator context.**
///
/// This includes SessionStart, PreToolUse, PostToolUse, Stop, and all other hooks.
/// The reason is simple: validator subagents should not trigger any AVP processing
/// at all - no file tracking, no validators, no cleanup. They are ephemeral agents
/// that exist solely to evaluate code quality, and their hooks must pass through
/// silently to prevent infinite recursion loops.
#[derive(Debug, Default, Clone)]
pub struct ValidatorContextStarter<I: HookInputType> {
    _phantom: PhantomData<I>,
}

impl<I: HookInputType> ValidatorContextStarter<I> {
    /// Create a new validator context starter.
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Check if we're running inside a validator context.
    fn in_validator_context() -> bool {
        std::env::var("CLAUDE_ACP").is_ok()
    }
}

impl<I: HookInputType> ChainStarter<I> for ValidatorContextStarter<I> {
    fn start(&self, _input: &I, ctx: &mut ChainContext) -> Result<StarterResult, ChainError> {
        if Self::in_validator_context() {
            tracing::debug!(
                "ValidatorContextStarter: Skipping chain - running inside validator context"
            );
            ctx.set_exit_code(0);
            // Return pass-through output - allow the hook to continue
            Ok(StarterResult::Stop(ChainOutput::success()))
        } else {
            ctx.set_exit_code(0);
            Ok(StarterResult::Continue)
        }
    }

    fn exit_code(&self) -> i32 {
        0
    }

    fn name(&self) -> &'static str {
        "ValidatorContextStarter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PreToolUseInput;

    fn make_input() -> PreToolUseInput {
        serde_json::from_value(serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        }))
        .unwrap()
    }

    #[test]
    fn test_success_starter() {
        let starter: SuccessStarter<PreToolUseInput> = SuccessStarter::new();
        let input = make_input();
        let mut ctx = ChainContext::new();

        match starter.start(&input, &mut ctx).unwrap() {
            StarterResult::Continue => {}
            _ => panic!("Expected Continue"),
        }
        assert_eq!(ctx.exit_code(), 0);
    }

    #[test]
    fn test_blocking_error_starter() {
        let starter: BlockingErrorStarter<PreToolUseInput> =
            BlockingErrorStarter::new("Test reason");
        let input = make_input();
        let mut ctx = ChainContext::new();

        match starter.start(&input, &mut ctx).unwrap() {
            StarterResult::Stop(output) => {
                assert!(!output.continue_execution);
                assert_eq!(output.stop_reason, Some("Test reason".to_string()));
            }
            _ => panic!("Expected Stop"),
        }
        assert_eq!(ctx.exit_code(), VALIDATOR_BLOCK_EXIT_CODE);
    }

    #[test]
    fn test_conditional_starter_pass() {
        let starter = ConditionalStarter::new(
            |input: &PreToolUseInput| input.tool_name == "Bash",
            "Not Bash",
        );
        let input = make_input();
        let mut ctx = ChainContext::new();

        match starter.start(&input, &mut ctx).unwrap() {
            StarterResult::Continue => {}
            _ => panic!("Expected Continue"),
        }
    }

    #[test]
    fn test_conditional_starter_fail() {
        let starter = ConditionalStarter::new(
            |input: &PreToolUseInput| input.tool_name == "Write",
            "Not Write",
        );
        let input = make_input();
        let mut ctx = ChainContext::new();

        match starter.start(&input, &mut ctx).unwrap() {
            StarterResult::Stop(output) => {
                assert!(!output.continue_execution);
            }
            _ => panic!("Expected Stop"),
        }
    }
}
