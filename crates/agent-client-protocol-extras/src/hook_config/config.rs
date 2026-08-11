//! The config shapes a user writes, and the factory that makes runtime
//! registrations from them.
//!
//! The nesting has three levels, the same three Claude Code uses: an event
//! name gives a list of matcher groups, a matcher group gives an optional
//! matcher and a list of handler configs, and a handler config names a
//! command, a prompt, or an agent.
//!
//! [`HookConfig::build_registrations`] turns that tree into the
//! [`HookRegistration`] list the runtime dispatches on.

use super::decision::{HookEvaluator, HookHandler, HookRegistration, Matcher};
use super::event::{HookCommandContext, HookEventKind};
use super::handlers::{CommandHandler, EvaluatorHandler};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Top-level hook configuration, deserializable from JSON or YAML.
///
/// Matches Claude Code's format: event names are PascalCase keys in a map.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HookConfig {
    /// Event name → array of matcher groups
    #[serde(default)]
    pub hooks: HashMap<HookEventKindConfig, Vec<MatcherGroup>>,
}

/// A matcher group: optional regex filter + array of hook handlers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatcherGroup {
    /// Optional regex pattern to filter when hooks fire.
    /// Omit or use "*" to match all occurrences.
    #[serde(default)]
    pub matcher: Option<String>,
    /// Hook handlers to run when matched.
    pub hooks: Vec<HookHandlerConfig>,
}

/// Event kind identifiers — PascalCase matching Claude Code.
///
/// Includes forward-compatible variants for Claude Code events that ACP
/// cannot fire. These are silently skipped during `build_registrations()`,
/// allowing the same config file to work with both Claude Code and ACP.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEventKindConfig {
    /// Matches HookEventKind::SessionStart events.
    SessionStart,
    /// Matches HookEventKind::UserPromptSubmit events.
    UserPromptSubmit,
    /// Matches HookEventKind::PreToolUse events.
    PreToolUse,
    /// Matches HookEventKind::PostToolUse events.
    PostToolUse,
    /// Matches HookEventKind::PostToolUseFailure events.
    PostToolUseFailure,
    /// Matches HookEventKind::Stop events.
    Stop,
    /// Matches HookEventKind::Notification events.
    Notification,
    // Forward-compatible: not fired by ACP, silently skipped
    /// Forward-compatible: permission request event.
    PermissionRequest,
    /// Forward-compatible: subagent start event.
    SubagentStart,
    /// Forward-compatible: subagent stop event.
    SubagentStop,
    /// Forward-compatible: pre-compaction event.
    PreCompact,
    /// Forward-compatible: setup event.
    Setup,
    /// Forward-compatible: session end event.
    SessionEnd,
    /// Forward-compatible: teammate idle event.
    TeammateIdle,
    /// Forward-compatible: task completion event.
    TaskCompleted,
    /// Forward-compatible: MCP elicitation request.
    Elicitation,
    /// Forward-compatible: MCP elicitation response.
    ElicitationResult,
    /// Forward-compatible: instructions/rules files loaded.
    InstructionsLoaded,
    /// Forward-compatible: config files changed.
    ConfigChange,
    /// Forward-compatible: worktree created.
    WorktreeCreate,
    /// Forward-compatible: worktree removed.
    WorktreeRemove,
    /// Forward-compatible: after context compaction.
    PostCompact,
}

/// Error returned when a config event kind has no ACP equivalent.
#[derive(Clone, Debug)]
pub struct UnsupportedEventKind;

impl std::fmt::Display for UnsupportedEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("event kind is not supported by ACP")
    }
}

impl std::error::Error for UnsupportedEventKind {}

impl TryFrom<HookEventKindConfig> for HookEventKind {
    type Error = UnsupportedEventKind;

    /// Maps a supported `HookEventKindConfig` variant to its `HookEventKind`.
    /// Returns `UnsupportedEventKind` for forward-compatible variants that ACP does not support.
    ///
    /// Looks the config variant up in a table of supported `(HookEventKindConfig,
    /// HookEventKind)` pairs, rather than matching every variant by hand — a
    /// config variant not listed there is, by construction, one ACP does not
    /// support yet, so it falls through to `UnsupportedEventKind`.
    fn try_from(config: HookEventKindConfig) -> Result<Self, Self::Error> {
        /// Config variants ACP can fire, paired with their `HookEventKind`.
        /// Forward-compatible, Claude-Code-only variants (`PermissionRequest`,
        /// `SubagentStart`, `SubagentStop`, `PreCompact`, `Setup`, `SessionEnd`)
        /// are deliberately absent, and resolve to `UnsupportedEventKind` below.
        const SUPPORTED: &[(HookEventKindConfig, HookEventKind)] = &[
            (
                HookEventKindConfig::SessionStart,
                HookEventKind::SessionStart,
            ),
            (
                HookEventKindConfig::UserPromptSubmit,
                HookEventKind::UserPromptSubmit,
            ),
            (HookEventKindConfig::PreToolUse, HookEventKind::PreToolUse),
            (HookEventKindConfig::PostToolUse, HookEventKind::PostToolUse),
            (
                HookEventKindConfig::PostToolUseFailure,
                HookEventKind::PostToolUseFailure,
            ),
            (HookEventKindConfig::Stop, HookEventKind::Stop),
            (
                HookEventKindConfig::Notification,
                HookEventKind::Notification,
            ),
            (HookEventKindConfig::PostCompact, HookEventKind::PostCompact),
            (
                HookEventKindConfig::TeammateIdle,
                HookEventKind::TeammateIdle,
            ),
            (
                HookEventKindConfig::TaskCompleted,
                HookEventKind::TaskCompleted,
            ),
            (HookEventKindConfig::Elicitation, HookEventKind::Elicitation),
            (
                HookEventKindConfig::ElicitationResult,
                HookEventKind::ElicitationResult,
            ),
            (
                HookEventKindConfig::InstructionsLoaded,
                HookEventKind::InstructionsLoaded,
            ),
            (
                HookEventKindConfig::ConfigChange,
                HookEventKind::ConfigChange,
            ),
            (
                HookEventKindConfig::WorktreeCreate,
                HookEventKind::WorktreeCreate,
            ),
            (
                HookEventKindConfig::WorktreeRemove,
                HookEventKind::WorktreeRemove,
            ),
        ];

        SUPPORTED
            .iter()
            .find(|(from, _)| *from == config)
            .map(|(_, to)| *to)
            .ok_or(UnsupportedEventKind)
    }
}

/// Handler configuration — only 3 types matching Claude Code.
///
/// - `command` — run a shell command, interpret exit code + JSON stdout
/// - `prompt` — send a prompt to an LLM for single-turn evaluation
/// - `agent` — spawn an agent with tool access for multi-turn evaluation
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookHandlerConfig {
    /// Run a shell command with JSON stdin/stdout protocol.
    Command {
        /// Shell command to execute.
        command: String,
        /// Timeout in seconds (default 600).
        #[serde(default = "default_command_timeout")]
        timeout: u64,
    },
    /// Send a prompt to an LLM for single-turn evaluation.
    Prompt {
        /// Prompt text. Use `$ARGUMENTS` as placeholder for hook input JSON.
        prompt: String,
        /// Optional model identifier.
        #[serde(default)]
        model: Option<String>,
        /// Timeout in seconds (default 30).
        #[serde(default = "default_prompt_timeout")]
        timeout: u64,
    },
    /// Spawn an agent with tool access for multi-turn evaluation.
    Agent {
        /// Prompt text. Use `$ARGUMENTS` as placeholder for hook input JSON.
        prompt: String,
        /// Optional model identifier.
        #[serde(default)]
        model: Option<String>,
        /// Timeout in seconds (default 60).
        #[serde(default = "default_agent_timeout")]
        timeout: u64,
    },
}

/// Default timeout, in seconds, for `command` hook handlers.
const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 600;

/// Default timeout, in seconds, for `agent` hook handlers.
const DEFAULT_AGENT_TIMEOUT_SECS: u64 = 60;

/// Default timeout, in seconds, for `prompt` hook handlers.
const DEFAULT_PROMPT_TIMEOUT_SECS: u64 = 30;

fn default_command_timeout() -> u64 {
    DEFAULT_COMMAND_TIMEOUT_SECS
}

fn default_prompt_timeout() -> u64 {
    DEFAULT_PROMPT_TIMEOUT_SECS
}

fn default_agent_timeout() -> u64 {
    DEFAULT_AGENT_TIMEOUT_SECS
}

// ---------------------------------------------------------------------------
// Config errors
// ---------------------------------------------------------------------------

/// Error building hook registrations from config.
#[derive(Debug, thiserror::Error)]
pub enum HookConfigError {
    /// Invalid regex pattern in hook matcher.
    #[error("invalid regex pattern in hook matcher: {0}")]
    InvalidRegex(#[from] regex::Error),
    /// Hook entry has empty hooks list.
    #[error("hook entry has empty hooks list")]
    EmptyHooks,
    /// Prompt or agent hook requires an evaluator but none was provided.
    #[error("prompt or agent hook requires a HookEvaluator, but none was provided")]
    MissingEvaluator,
}

// ---------------------------------------------------------------------------
// Factory: config → registrations
// ---------------------------------------------------------------------------

/// Build a handler from config, requiring an evaluator for prompt/agent types.
///
/// `command_context` is captured into the handler so its command/prompt JSON
/// stdin carries the AVP context fields (`transcript_path`, `permission_mode`).
fn build_handler(
    config: &HookHandlerConfig,
    evaluator: &Option<Arc<dyn HookEvaluator>>,
    command_context: &HookCommandContext,
) -> Result<Arc<dyn HookHandler>, HookConfigError> {
    match config {
        HookHandlerConfig::Command { command, timeout } => Ok(Arc::new(CommandHandler {
            command: command.clone(),
            timeout: std::time::Duration::from_secs(*timeout),
            command_context: command_context.clone(),
        })),
        HookHandlerConfig::Prompt {
            prompt, timeout, ..
        } => build_evaluator_handler(prompt, *timeout, evaluator, command_context, false),
        HookHandlerConfig::Agent {
            prompt, timeout, ..
        } => build_evaluator_handler(prompt, *timeout, evaluator, command_context, true),
    }
}

/// Build the shared [`EvaluatorHandler`] backing both `type: prompt`
/// (`is_agent=false`) and `type: agent` (`is_agent=true`) hook configs.
///
/// Both config variants carry the same `prompt`/`timeout` shape and both
/// require an evaluator; `is_agent` is the only behavioral difference, so
/// this is the single construction site for both — extracted so the two
/// branches of [`build_handler`] cannot drift out of sync.
fn build_evaluator_handler(
    prompt: &str,
    timeout: u64,
    evaluator: &Option<Arc<dyn HookEvaluator>>,
    command_context: &HookCommandContext,
    is_agent: bool,
) -> Result<Arc<dyn HookHandler>, HookConfigError> {
    let eval = evaluator
        .as_ref()
        .ok_or(HookConfigError::MissingEvaluator)?
        .clone();
    Ok(Arc::new(EvaluatorHandler {
        prompt_template: prompt.to_string(),
        evaluator: eval,
        timeout: std::time::Duration::from_secs(timeout),
        command_context: command_context.clone(),
        is_agent,
    }))
}

impl HookConfig {
    /// Build runtime [`HookRegistration`]s from this config.
    ///
    /// Each matcher group + handler combination becomes one `HookRegistration`.
    /// Prompt/agent handlers require an evaluator.
    ///
    /// Command/prompt hooks built this way carry the *default* (empty)
    /// [`HookCommandContext`], so their JSON stdin has an empty `transcript_path`
    /// and the default permission mode. Use
    /// [`build_registrations_with_context`](Self::build_registrations_with_context)
    /// to supply a real context.
    pub fn build_registrations(
        &self,
        evaluator: Option<Arc<dyn HookEvaluator>>,
    ) -> Result<Vec<HookRegistration>, HookConfigError> {
        self.build_registrations_with_context(evaluator, &HookCommandContext::default())
    }

    /// Build runtime [`HookRegistration`]s carrying an explicit command context.
    ///
    /// Identical to [`build_registrations`](Self::build_registrations) but the
    /// supplied `command_context` is folded into every command/prompt/agent
    /// handler's JSON stdin (as `transcript_path` and, when set,
    /// `permission_mode`), so hooks observe the same input shape Claude Code
    /// sends.
    pub fn build_registrations_with_context(
        &self,
        evaluator: Option<Arc<dyn HookEvaluator>>,
        command_context: &HookCommandContext,
    ) -> Result<Vec<HookRegistration>, HookConfigError> {
        let mut registrations = Vec::new();

        for (event_kind_config, matcher_groups) in &self.hooks {
            let event_kind: HookEventKind = match event_kind_config.clone().try_into() {
                Ok(kind) => kind,
                Err(_) => continue, // Skip forward-compatible event kinds
            };

            for group in matcher_groups {
                if group.hooks.is_empty() {
                    return Err(HookConfigError::EmptyHooks);
                }

                let matcher = Matcher::try_parse(group.matcher.as_deref().unwrap_or(""))?;

                for handler_config in &group.hooks {
                    let handler = build_handler(handler_config, &evaluator, command_context)?;
                    registrations.push(HookRegistration::new(
                        vec![event_kind],
                        matcher.clone(),
                        handler,
                    ));
                }
            }
        }

        Ok(registrations)
    }
}

#[cfg(test)]
mod tests;
