//! Agent-agnostic AVP output types for each hook type.
//!
//! These types represent what chains and validators produce internally.
//! Each agent strategy (e.g., ClaudeCodeHookStrategy) transforms these
//! into agent-specific formats.

/// Result from validator execution that blocked.
#[derive(Debug, Clone)]
pub struct ValidatorBlock {
    /// Name of the validator that blocked.
    pub validator_name: String,
    /// Message explaining why it blocked.
    pub message: String,
}

/// Base fields common to all AVP outputs.
#[derive(Debug, Clone, Default)]
pub struct AvpOutputBase {
    /// Whether to continue execution.
    pub should_continue: bool,
    /// Reason for stopping (if should_continue is false).
    pub stop_reason: Option<String>,
    /// Validator that blocked (if any).
    pub validator_block: Option<ValidatorBlock>,
    /// System message to inject into context.
    pub system_message: Option<String>,
    /// Whether to suppress output display.
    pub suppress_output: bool,
}

impl AvpOutputBase {
    /// Create a success output (allow continuation).
    pub fn success() -> Self {
        Self {
            should_continue: true,
            ..Default::default()
        }
    }

    /// Create a blocked output from a validator.
    pub fn blocked(validator_name: impl Into<String>, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            should_continue: false,
            stop_reason: Some(message.clone()),
            validator_block: Some(ValidatorBlock {
                validator_name: validator_name.into(),
                message,
            }),
            ..Default::default()
        }
    }
}

// ============================================================================
// Macros for DRY impl blocks
// ============================================================================

/// Implement the allow/deny pattern for output types.
///
/// Used by types that have:
/// - A boolean "allow" field (which may be named differently, e.g. `allow` or `grant`)
/// - A `deny_reason: Option<String>` field
/// - `allow()`, `deny()`, and `deny_from_validator()` methods
///
/// Parameters:
/// - `$ty`: the struct type name
/// - `$allow_field`: the name of the boolean allow field
/// - `$deny_field`: the name of the deny reason field (`deny_reason`)
macro_rules! impl_allow_deny {
    ($ty:ty, $allow_field:ident, $deny_field:ident) => {
        impl $ty {
            /// Create an allow output.
            pub fn allow() -> Self {
                Self::default()
            }

            /// Create a deny output.
            pub fn deny(reason: impl Into<String>) -> Self {
                let reason = reason.into();
                Self {
                    base: AvpOutputBase {
                        should_continue: true,
                        ..Default::default()
                    },
                    $allow_field: false,
                    $deny_field: Some(reason),
                }
            }

            /// Create a deny output from a validator block.
            pub fn deny_from_validator(
                validator_name: impl Into<String>,
                message: impl Into<String>,
            ) -> Self {
                let validator_name = validator_name.into();
                let message = message.into();
                Self {
                    base: AvpOutputBase {
                        should_continue: true,
                        validator_block: Some(ValidatorBlock {
                            validator_name,
                            message: message.clone(),
                        }),
                        ..Default::default()
                    },
                    $allow_field: false,
                    $deny_field: Some(message),
                }
            }
        }
    };
}

/// Implement the allow/block pattern for output types.
///
/// Used by types that have:
/// - A boolean "allow" field (which may be named `allow`, `allow_stop`, or `allow_idle`)
/// - A `block_reason: Option<String>` field
/// - `allow()`, `block()`, and optionally `block_from_validator()` methods
///
/// Parameters:
/// - `$ty`: the struct type name
/// - `$allow_field`: the name of the boolean allow field
/// - `$block_field`: the name of the block reason field (`block_reason`)
/// - `with_validator` (optional): when present, also generates `block_from_validator()`
macro_rules! impl_allow_block {
    ($ty:ty, $allow_field:ident, $block_field:ident) => {
        impl $ty {
            /// Create an allow output.
            pub fn allow() -> Self {
                Self::default()
            }

            /// Create a block output.
            pub fn block(reason: impl Into<String>) -> Self {
                let reason = reason.into();
                Self {
                    base: AvpOutputBase {
                        should_continue: true,
                        ..Default::default()
                    },
                    $allow_field: false,
                    $block_field: Some(reason),
                }
            }
        }
    };
    ($ty:ty, $allow_field:ident, $block_field:ident, with_validator) => {
        impl_allow_block!($ty, $allow_field, $block_field);

        impl $ty {
            /// Create a block output from a validator.
            pub fn block_from_validator(
                validator_name: impl Into<String>,
                message: impl Into<String>,
            ) -> Self {
                let validator_name = validator_name.into();
                let message = message.into();
                Self {
                    base: AvpOutputBase {
                        should_continue: true,
                        validator_block: Some(ValidatorBlock {
                            validator_name,
                            message: message.clone(),
                        }),
                        ..Default::default()
                    },
                    $allow_field: false,
                    $block_field: Some(message),
                }
            }
        }
    };
}

/// Implement the accept/flag pattern for output types.
///
/// Used by types that have:
/// - A `flagged: bool` field
/// - A `feedback: Option<String>` field
/// - `accept()`, `flag()`, and optionally `flag_from_validator()` methods
///
/// Parameters:
/// - `$ty`: the struct type name
/// - `with_validator` (optional): when present, also generates `flag_from_validator()`
macro_rules! impl_accept_flag {
    ($ty:ty) => {
        impl $ty {
            /// Create an accept output (no issues).
            pub fn accept() -> Self {
                Self::default()
            }

            /// Create a flagged output (has issues).
            pub fn flag(feedback: impl Into<String>) -> Self {
                Self {
                    base: AvpOutputBase::success(),
                    flagged: true,
                    feedback: Some(feedback.into()),
                }
            }
        }
    };
    ($ty:ty, with_validator) => {
        impl_accept_flag!($ty);

        impl $ty {
            /// Create a flagged output from a validator block.
            pub fn flag_from_validator(
                validator_name: impl Into<String>,
                message: impl Into<String>,
            ) -> Self {
                let validator_name = validator_name.into();
                let message = message.into();
                Self {
                    base: AvpOutputBase {
                        should_continue: true,
                        validator_block: Some(ValidatorBlock {
                            validator_name,
                            message: message.clone(),
                        }),
                        ..Default::default()
                    },
                    flagged: true,
                    feedback: Some(message),
                }
            }
        }
    };
}

/// Implement the observe-only pattern for output types.
///
/// Used by types that have only a `base: AvpOutputBase` field and a `success()` method.
///
/// Parameters:
/// - `$ty`: the struct type name
macro_rules! impl_observe_only {
    ($ty:ty) => {
        impl $ty {
            /// Create a success output.
            pub fn success() -> Self {
                Self {
                    base: AvpOutputBase::success(),
                }
            }
        }
    };
}

// ============================================================================
// Per-hook AVP output types
// ============================================================================

/// AVP output for PreToolUse hook.
///
/// Represents the result of validating a tool call before execution.
#[derive(Debug, Clone)]
pub struct AvpPreToolUseOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the tool call.
    pub allow: bool,
    /// Reason for deny (if not allowed).
    pub deny_reason: Option<String>,
}

impl Default for AvpPreToolUseOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow: true,
            deny_reason: None,
        }
    }
}

impl_allow_deny!(AvpPreToolUseOutput, allow, deny_reason);

/// AVP output for PermissionRequest hook.
///
/// Represents the result of processing a permission request.
#[derive(Debug, Clone)]
pub struct AvpPermissionRequestOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to grant permission.
    pub grant: bool,
    /// Reason for deny (if not granted).
    pub deny_reason: Option<String>,
}

impl Default for AvpPermissionRequestOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            grant: true,
            deny_reason: None,
        }
    }
}

impl_allow_deny!(AvpPermissionRequestOutput, grant, deny_reason);

/// AVP output for PostToolUse hook.
///
/// Represents the result of validating a tool result after execution.
#[derive(Debug, Clone)]
pub struct AvpPostToolUseOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether the tool result is flagged (has issues).
    pub flagged: bool,
    /// Feedback about the issue (if flagged).
    pub feedback: Option<String>,
}

impl Default for AvpPostToolUseOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            flagged: false,
            feedback: None,
        }
    }
}

impl_accept_flag!(AvpPostToolUseOutput, with_validator);

/// AVP output for Stop hook.
///
/// Represents the result of validating an agent stop request.
#[derive(Debug, Clone)]
pub struct AvpStopOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the stop.
    pub allow_stop: bool,
    /// Reason for blocking stop (if not allowed).
    pub block_reason: Option<String>,
}

impl Default for AvpStopOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow_stop: true,
            block_reason: None,
        }
    }
}

impl_allow_block!(AvpStopOutput, allow_stop, block_reason, with_validator);

/// AVP output for UserPromptSubmit hook.
///
/// Represents the result of validating a user prompt.
#[derive(Debug, Clone)]
pub struct AvpUserPromptSubmitOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the prompt.
    pub allow: bool,
    /// Reason for blocking (if not allowed).
    pub block_reason: Option<String>,
}

impl Default for AvpUserPromptSubmitOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow: true,
            block_reason: None,
        }
    }
}

impl_allow_block!(AvpUserPromptSubmitOutput, allow, block_reason);

/// AVP output for SessionStart hook.
#[derive(Debug, Clone, Default)]
pub struct AvpSessionStartOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpSessionStartOutput);

/// AVP output for SessionEnd hook.
#[derive(Debug, Clone, Default)]
pub struct AvpSessionEndOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpSessionEndOutput);

/// AVP output for Notification hook.
#[derive(Debug, Clone, Default)]
pub struct AvpNotificationOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpNotificationOutput);

/// AVP output for SubagentStart hook.
#[derive(Debug, Clone, Default)]
pub struct AvpSubagentStartOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpSubagentStartOutput);

/// AVP output for SubagentStop hook.
#[derive(Debug, Clone)]
pub struct AvpSubagentStopOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the subagent stop.
    pub allow_stop: bool,
    /// Reason for blocking stop (if not allowed).
    pub block_reason: Option<String>,
}

impl Default for AvpSubagentStopOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow_stop: true,
            block_reason: None,
        }
    }
}

impl_allow_block!(AvpSubagentStopOutput, allow_stop, block_reason);

/// AVP output for PreCompact hook.
#[derive(Debug, Clone, Default)]
pub struct AvpPreCompactOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpPreCompactOutput);

/// AVP output for Setup hook.
#[derive(Debug, Clone, Default)]
pub struct AvpSetupOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpSetupOutput);

/// AVP output for PostToolUseFailure hook.
#[derive(Debug, Clone)]
pub struct AvpPostToolUseFailureOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether the failure is flagged (has additional issues).
    pub flagged: bool,
    /// Feedback about the issue (if flagged).
    pub feedback: Option<String>,
}

impl Default for AvpPostToolUseFailureOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            flagged: false,
            feedback: None,
        }
    }
}

impl_accept_flag!(AvpPostToolUseFailureOutput);

/// AVP output for Elicitation hook.
///
/// Represents the result of validating an MCP elicitation request.
#[derive(Debug, Clone)]
pub struct AvpElicitationOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the elicitation.
    pub allow: bool,
    /// Reason for deny (if not allowed).
    pub deny_reason: Option<String>,
}

impl Default for AvpElicitationOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow: true,
            deny_reason: None,
        }
    }
}

impl_allow_deny!(AvpElicitationOutput, allow, deny_reason);

/// AVP output for ElicitationResult hook.
///
/// Represents the result of validating a user's elicitation response.
#[derive(Debug, Clone)]
pub struct AvpElicitationResultOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the response.
    pub allow: bool,
    /// Reason for blocking (if not allowed).
    pub block_reason: Option<String>,
}

impl Default for AvpElicitationResultOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow: true,
            block_reason: None,
        }
    }
}

impl_allow_block!(
    AvpElicitationResultOutput,
    allow,
    block_reason,
    with_validator
);

/// AVP output for InstructionsLoaded hook.
///
/// Observe-only — cannot block.
#[derive(Debug, Clone, Default)]
pub struct AvpInstructionsLoadedOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpInstructionsLoadedOutput);

/// AVP output for ConfigChange hook.
///
/// Represents the result of validating a config change.
#[derive(Debug, Clone)]
pub struct AvpConfigChangeOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the config change.
    pub allow: bool,
    /// Reason for blocking (if not allowed).
    pub block_reason: Option<String>,
}

impl Default for AvpConfigChangeOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow: true,
            block_reason: None,
        }
    }
}

impl_allow_block!(AvpConfigChangeOutput, allow, block_reason, with_validator);

/// AVP output for WorktreeCreate hook.
///
/// Represents the result of validating worktree creation.
#[derive(Debug, Clone)]
pub struct AvpWorktreeCreateOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow worktree creation.
    pub allow: bool,
    /// Reason for deny (if not allowed).
    pub deny_reason: Option<String>,
}

impl Default for AvpWorktreeCreateOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow: true,
            deny_reason: None,
        }
    }
}

impl_allow_deny!(AvpWorktreeCreateOutput, allow, deny_reason);

/// AVP output for WorktreeRemove hook.
///
/// Observe-only — cannot block.
#[derive(Debug, Clone, Default)]
pub struct AvpWorktreeRemoveOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpWorktreeRemoveOutput);

/// AVP output for PostCompact hook.
///
/// Observe-only — cannot block.
#[derive(Debug, Clone, Default)]
pub struct AvpPostCompactOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
}

impl_observe_only!(AvpPostCompactOutput);

/// AVP output for TeammateIdle hook.
///
/// Represents the result of validating a teammate idle event.
#[derive(Debug, Clone)]
pub struct AvpTeammateIdleOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the teammate to go idle.
    pub allow_idle: bool,
    /// Reason for blocking idle (if not allowed).
    pub block_reason: Option<String>,
}

impl Default for AvpTeammateIdleOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow_idle: true,
            block_reason: None,
        }
    }
}

impl_allow_block!(
    AvpTeammateIdleOutput,
    allow_idle,
    block_reason,
    with_validator
);

/// AVP output for TaskCompleted hook.
///
/// Represents the result of validating a task completion.
#[derive(Debug, Clone)]
pub struct AvpTaskCompletedOutput {
    /// Base output fields.
    pub base: AvpOutputBase,
    /// Whether to allow the task completion.
    pub allow: bool,
    /// Reason for blocking (if not allowed).
    pub block_reason: Option<String>,
}

impl Default for AvpTaskCompletedOutput {
    fn default() -> Self {
        Self {
            base: AvpOutputBase::success(),
            allow: true,
            block_reason: None,
        }
    }
}

impl_allow_block!(AvpTaskCompletedOutput, allow, block_reason, with_validator);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_tool_use_allow() {
        let output = AvpPreToolUseOutput::allow();
        assert!(output.allow);
        assert!(output.deny_reason.is_none());
        assert!(output.base.should_continue);
    }

    #[test]
    fn test_pre_tool_use_deny() {
        let output = AvpPreToolUseOutput::deny("Command not allowed");
        assert!(!output.allow);
        assert_eq!(output.deny_reason.as_deref(), Some("Command not allowed"));
        assert!(output.base.should_continue); // Agent continues even when tool denied
    }

    #[test]
    fn test_pre_tool_use_deny_from_validator() {
        let output =
            AvpPreToolUseOutput::deny_from_validator("input-validation", "rm -rf not allowed");
        assert!(!output.allow);
        assert!(output.base.validator_block.is_some());
        let block = output.base.validator_block.as_ref().unwrap();
        assert_eq!(block.validator_name, "input-validation");
    }

    #[test]
    fn test_post_tool_use_accept() {
        let output = AvpPostToolUseOutput::accept();
        assert!(!output.flagged);
        assert!(output.feedback.is_none());
    }

    #[test]
    fn test_post_tool_use_flag() {
        let output = AvpPostToolUseOutput::flag("Found hardcoded secrets");
        assert!(output.flagged);
        assert_eq!(output.feedback.as_deref(), Some("Found hardcoded secrets"));
    }

    #[test]
    fn test_stop_allow() {
        let output = AvpStopOutput::allow();
        assert!(output.allow_stop);
        assert!(output.block_reason.is_none());
    }

    #[test]
    fn test_stop_block() {
        let output = AvpStopOutput::block("Must fix tests before stopping");
        assert!(!output.allow_stop);
        assert_eq!(
            output.block_reason.as_deref(),
            Some("Must fix tests before stopping")
        );
        assert!(output.base.should_continue); // Force agent to continue
    }

    // --- New output type tests ---

    #[test]
    fn test_elicitation_allow() {
        let output = AvpElicitationOutput::allow();
        assert!(output.allow);
        assert!(output.deny_reason.is_none());
        assert!(output.base.should_continue);
    }

    #[test]
    fn test_elicitation_deny() {
        let output = AvpElicitationOutput::deny("Not allowed");
        assert!(!output.allow);
        assert_eq!(output.deny_reason.as_deref(), Some("Not allowed"));
    }

    #[test]
    fn test_elicitation_deny_from_validator() {
        let output = AvpElicitationOutput::deny_from_validator("checker", "blocked");
        assert!(!output.allow);
        assert!(output.base.validator_block.is_some());
        assert_eq!(
            output.base.validator_block.as_ref().unwrap().validator_name,
            "checker"
        );
    }

    #[test]
    fn test_elicitation_result_allow() {
        let output = AvpElicitationResultOutput::allow();
        assert!(output.allow);
        assert!(output.block_reason.is_none());
    }

    #[test]
    fn test_elicitation_result_block() {
        let output = AvpElicitationResultOutput::block("invalid response");
        assert!(!output.allow);
        assert_eq!(output.block_reason.as_deref(), Some("invalid response"));
    }

    #[test]
    fn test_observe_only_outputs() {
        let il = AvpInstructionsLoadedOutput::success();
        assert!(il.base.should_continue);
        let wr = AvpWorktreeRemoveOutput::success();
        assert!(wr.base.should_continue);
        let pc = AvpPostCompactOutput::success();
        assert!(pc.base.should_continue);
    }

    #[test]
    fn test_config_change_allow_and_block() {
        let allow = AvpConfigChangeOutput::allow();
        assert!(allow.allow);
        let block = AvpConfigChangeOutput::block("dangerous setting");
        assert!(!block.allow);
        assert_eq!(block.block_reason.as_deref(), Some("dangerous setting"));
    }

    #[test]
    fn test_worktree_create_allow_and_deny() {
        let allow = AvpWorktreeCreateOutput::allow();
        assert!(allow.allow);
        let deny = AvpWorktreeCreateOutput::deny("too many worktrees");
        assert!(!deny.allow);
        assert_eq!(deny.deny_reason.as_deref(), Some("too many worktrees"));
    }

    #[test]
    fn test_worktree_create_deny_from_validator() {
        let output =
            AvpWorktreeCreateOutput::deny_from_validator("branch-policy", "invalid branch name");
        assert!(!output.allow);
        assert!(output.base.validator_block.is_some());
        assert_eq!(
            output.base.validator_block.as_ref().unwrap().validator_name,
            "branch-policy"
        );
    }

    #[test]
    fn test_teammate_idle_allow_and_block() {
        let allow = AvpTeammateIdleOutput::allow();
        assert!(allow.allow_idle);
        let block = AvpTeammateIdleOutput::block("work remains");
        assert!(!block.allow_idle);
        assert_eq!(block.block_reason.as_deref(), Some("work remains"));
    }

    #[test]
    fn test_elicitation_result_block_from_validator() {
        let output =
            AvpElicitationResultOutput::block_from_validator("checker", "response blocked");
        assert!(!output.allow);
        assert!(output.base.validator_block.is_some());
        let block = output.base.validator_block.as_ref().unwrap();
        assert_eq!(block.validator_name, "checker");
        assert_eq!(block.message, "response blocked");
    }

    #[test]
    fn test_config_change_block_from_validator() {
        let output = AvpConfigChangeOutput::block_from_validator("policy", "disallowed setting");
        assert!(!output.allow);
        assert!(output.base.validator_block.is_some());
        assert_eq!(
            output.base.validator_block.as_ref().unwrap().validator_name,
            "policy"
        );
    }

    #[test]
    fn test_teammate_idle_block_from_validator() {
        let output = AvpTeammateIdleOutput::block_from_validator("ralph", "work remains");
        assert!(!output.allow_idle);
        assert!(output.base.validator_block.is_some());
        assert_eq!(
            output.base.validator_block.as_ref().unwrap().validator_name,
            "ralph"
        );
    }

    #[test]
    fn test_task_completed_block_from_validator() {
        let output = AvpTaskCompletedOutput::block_from_validator("qa-check", "tests not passing");
        assert!(!output.allow);
        assert!(output.base.validator_block.is_some());
        assert_eq!(
            output.base.validator_block.as_ref().unwrap().validator_name,
            "qa-check"
        );
    }

    #[test]
    fn test_task_completed_allow_and_block() {
        let allow = AvpTaskCompletedOutput::allow();
        assert!(allow.allow);
        let block = AvpTaskCompletedOutput::block("tests not passing");
        assert!(!block.allow);
        assert_eq!(block.block_reason.as_deref(), Some("tests not passing"));
    }
}
