//! Hook configuration — Claude-compatible declarative hook registration
//!
//! Matches Claude Code's 3-level config nesting:
//! 1. Event name (PascalCase) → array of matcher groups
//! 2. Matcher group → optional regex matcher + array of handlers
//! 3. Handler → command, prompt, or agent
//!
//! JSON example (Claude Code format):
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "matcher": "Bash",
//!         "hooks": [
//!           { "type": "command", "command": "./check.sh" }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! YAML example:
//! ```yaml
//! hooks:
//!   PreToolUse:
//!     - matcher: "Bash"
//!       hooks:
//!         - type: command
//!           command: "./check.sh"
//! ```

use crate::hookable_agent::{
    HookDecision, HookEvent, HookEventKind, HookHandler, HookRegistration,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Config types (3-level nesting matching Claude Code)
// ---------------------------------------------------------------------------

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
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    Notification,
    // Forward-compatible: not fired by ACP, silently skipped
    PermissionRequest,
    SubagentStart,
    SubagentStop,
    PreCompact,
    Setup,
    SessionEnd,
    TeammateIdle,
    TaskCompleted,
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

    fn try_from(config: HookEventKindConfig) -> Result<Self, Self::Error> {
        match config {
            HookEventKindConfig::SessionStart => Ok(HookEventKind::SessionStart),
            HookEventKindConfig::UserPromptSubmit => Ok(HookEventKind::UserPromptSubmit),
            HookEventKindConfig::PreToolUse => Ok(HookEventKind::PreToolUse),
            HookEventKindConfig::PostToolUse => Ok(HookEventKind::PostToolUse),
            HookEventKindConfig::PostToolUseFailure => Ok(HookEventKind::PostToolUseFailure),
            HookEventKindConfig::Stop => Ok(HookEventKind::Stop),
            HookEventKindConfig::Notification => Ok(HookEventKind::Notification),
            HookEventKindConfig::PermissionRequest
            | HookEventKindConfig::SubagentStart
            | HookEventKindConfig::SubagentStop
            | HookEventKindConfig::PreCompact
            | HookEventKindConfig::Setup
            | HookEventKindConfig::SessionEnd
            | HookEventKindConfig::TeammateIdle
            | HookEventKindConfig::TaskCompleted => Err(UnsupportedEventKind),
        }
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

fn default_command_timeout() -> u64 {
    600
}

fn default_prompt_timeout() -> u64 {
    30
}

fn default_agent_timeout() -> u64 {
    60
}

// ---------------------------------------------------------------------------
// Hook output types (Claude-compatible JSON parsing)
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

/// Decision values for top-level and permission decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookDecisionValue {
    /// Allow the action.
    Allow,
    /// Block/deny the action.
    Block,
    /// Ask user for permission (permission decisions only).
    Ask,
}

/// Parsed JSON output from a command hook's stdout.
///
/// Field names use camelCase to match Claude Code's JSON format.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    /// If false, stop Claude processing entirely. Takes precedence over other fields.
    #[serde(rename = "continue", default = "default_true")]
    pub should_continue: bool,
    /// Message shown to user when `should_continue` is false.
    pub stop_reason: Option<String>,
    /// If true, hide stdout from verbose output.
    #[serde(default)]
    pub suppress_output: bool,
    /// Warning message shown to the user.
    pub system_message: Option<String>,
    /// Top-level decision: "block" to prevent the action.
    pub decision: Option<HookDecisionValue>,
    /// Reason for the decision.
    pub reason: Option<String>,
    /// Event-specific output for richer control.
    pub hook_specific_output: Option<HookSpecificOutput>,
    /// Additional context string added to Claude's context.
    pub additional_context: Option<String>,
}

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            should_continue: true,
            stop_reason: None,
            suppress_output: false,
            system_message: None,
            decision: None,
            reason: None,
            hook_specific_output: None,
            additional_context: None,
        }
    }
}

/// Event-specific output fields inside `hookSpecificOutput`.
///
/// Tagged by `hookEventName` to enforce per-event field sets, matching
/// AVP's `#[serde(tag = "hookEventName")]` convention.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "hookEventName")]
pub enum HookSpecificOutput {
    PreToolUse {
        #[serde(rename = "permissionDecision")]
        permission_decision: Option<String>,
        #[serde(rename = "permissionDecisionReason")]
        permission_decision_reason: Option<String>,
        #[serde(rename = "updatedInput")]
        updated_input: Option<serde_json::Value>,
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    PostToolUse {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    PostToolUseFailure {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    UserPromptSubmit {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    Stop {
        reason: Option<String>,
    },
    SessionStart {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    Notification {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
}

/// Parsed JSON response from a prompt/agent hook evaluator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptHookResponse {
    /// true to allow, false to block/prevent stopping.
    pub ok: bool,
    /// Reason for blocking (required when ok is false).
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// HookEvaluator trait (for prompt/agent hooks)
// ---------------------------------------------------------------------------

/// Evaluator for prompt-based and agent-based hooks.
///
/// Callers implement this with their own LLM client.
/// For "prompt" hooks: single-turn evaluation (is_agent=false).
/// For "agent" hooks: multi-turn evaluation with tool access (is_agent=true).
#[async_trait::async_trait]
pub trait HookEvaluator: Send + Sync {
    /// Evaluate a prompt and return a JSON response string.
    ///
    /// Expected response format: `{ "ok": true }` or `{ "ok": false, "reason": "..." }`
    async fn evaluate(&self, prompt: &str, is_agent: bool) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// Config errors
// ---------------------------------------------------------------------------

/// Error building hook registrations from config.
#[derive(Debug, thiserror::Error)]
pub enum HookConfigError {
    #[error("Invalid regex pattern in hook matcher: {0}")]
    InvalidRegex(#[from] regex::Error),
    #[error("Hook entry has empty hooks list")]
    EmptyHooks,
    #[error("Prompt or agent hook requires a HookEvaluator, but none was provided")]
    MissingEvaluator,
}

// ---------------------------------------------------------------------------
// Built-in handlers
// ---------------------------------------------------------------------------

/// Command handler: runs shell command with JSON stdin/stdout protocol.
///
/// Exit codes (following Claude Code):
/// - 0 → parse stdout as HookOutput JSON, interpret based on event
/// - 2 → Block (stderr becomes reason)
/// - Other → Allow (warning logged)
struct CommandHandler {
    command: String,
    timeout: std::time::Duration,
}

#[async_trait::async_trait]
impl HookHandler for CommandHandler {
    async fn handle(&self, event: &HookEvent) -> HookDecision {
        let stdin_json = event.to_command_input().to_string();
        match run_command(&self.command, &stdin_json, self.timeout).await {
            Ok(output) => interpret_exit_code(&output, &self.command, event.kind()),
            Err(CommandRunError::SpawnFailed(e)) => {
                tracing::error!(command = %self.command, error = %e, "Hook command failed to execute");
                HookDecision::Allow
            }
            Err(CommandRunError::TimedOut) => {
                tracing::error!(command = %self.command, "Hook command timed out");
                HookDecision::Block {
                    reason: format!("Command '{}' timed out", self.command),
                }
            }
        }
    }
}

enum CommandRunError {
    SpawnFailed(std::io::Error),
    TimedOut,
}

/// Execute a hook command string via shell.
///
/// # Trust model
///
/// Hook commands come from admin-controlled configuration files (`.claude/settings.json`,
/// project `CLAUDE.md`, etc.) — the same trust model as Claude Code's hook system.
/// Shell execution via `sh -c` is intentional: hooks need pipes, redirects, and
/// multi-command chains. The config file itself is the trust boundary, not this function.
async fn run_command(
    command: &str,
    stdin_json: &str,
    timeout: std::time::Duration,
) -> Result<std::process::Output, CommandRunError> {
    use tokio::process::Command;

    let result = tokio::time::timeout(timeout, async {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(stdin_json.as_bytes()).await;
            drop(stdin);
        }

        child.wait_with_output().await
    })
    .await;

    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(CommandRunError::SpawnFailed(e)),
        Err(_) => Err(CommandRunError::TimedOut),
    }
}

/// Interpret a command's exit code into a HookDecision.
fn interpret_exit_code(
    output: &std::process::Output,
    command: &str,
    event_kind: HookEventKind,
) -> HookDecision {
    let code = output.status.code().unwrap_or(-1);
    match code {
        0 => interpret_exit_0_stdout(output, command, event_kind),
        2 => interpret_exit_2_stderr(output, command, event_kind),
        other => {
            tracing::warn!(
                command = %command,
                exit_code = other,
                "Hook command exited with unexpected code, allowing"
            );
            HookDecision::Allow
        }
    }
}

fn interpret_exit_0_stdout(
    output: &std::process::Output,
    command: &str,
    event_kind: HookEventKind,
) -> HookDecision {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();
    if stdout.is_empty() {
        return HookDecision::Allow;
    }
    match serde_json::from_str::<HookOutput>(stdout) {
        Ok(hook_output) => interpret_output(&hook_output, event_kind),
        Err(e) => {
            tracing::warn!(
                command = %command,
                error = %e,
                stdout = %stdout,
                "Failed to parse hook command JSON output, treating as Allow"
            );
            HookDecision::Allow
        }
    }
}

fn interpret_exit_2_stderr(
    output: &std::process::Output,
    command: &str,
    event_kind: HookEventKind,
) -> HookDecision {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let reason = if stderr.trim().is_empty() {
        format!("Command '{}' exited with code 2", command)
    } else {
        stderr.trim().to_string()
    };
    if is_blockable(event_kind) {
        HookDecision::Block { reason }
    } else if event_kind == HookEventKind::Stop {
        HookDecision::ShouldContinue { reason }
    } else if feeds_stderr_to_agent(event_kind) {
        HookDecision::AllowWithContext { context: reason }
    } else {
        tracing::warn!(
            command = %command,
            "Exit 2 on non-blockable event {:?}, treating as Allow",
            event_kind,
        );
        HookDecision::Allow
    }
}

/// Whether an event kind supports blocking via exit-2.
///
/// Only PreToolUse and UserPromptSubmit can block because the action
/// hasn't happened yet. All other events (PostToolUse, PostToolUseFailure,
/// Notification, SessionStart) cannot block.
fn is_blockable(kind: HookEventKind) -> bool {
    matches!(
        kind,
        HookEventKind::PreToolUse | HookEventKind::UserPromptSubmit
    )
}

/// Whether exit-2 stderr should be fed back as agent context.
///
/// PostToolUse and PostToolUseFailure can't block (action already happened)
/// but Claude Code feeds the stderr back to the agent as context.
fn feeds_stderr_to_agent(kind: HookEventKind) -> bool {
    matches!(
        kind,
        HookEventKind::PostToolUse | HookEventKind::PostToolUseFailure
    )
}

/// Prompt handler: calls HookEvaluator for single-turn LLM evaluation.
struct PromptHandler {
    prompt_template: String,
    evaluator: Arc<dyn HookEvaluator>,
    timeout: std::time::Duration,
}

#[async_trait::async_trait]
impl HookHandler for PromptHandler {
    async fn handle(&self, event: &HookEvent) -> HookDecision {
        let arguments_json = event.to_command_input().to_string();
        let prompt = self.prompt_template.replace("$ARGUMENTS", &arguments_json);

        let result = tokio::time::timeout(self.timeout, async {
            self.evaluator.evaluate(&prompt, false).await
        })
        .await;

        match result {
            Ok(Ok(response_json)) => {
                match serde_json::from_str::<PromptHookResponse>(&response_json) {
                    Ok(response) => interpret_prompt_response(&response, event.kind()),
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Failed to parse prompt hook response, treating as Allow"
                        );
                        HookDecision::Allow
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "Prompt hook evaluator failed");
                HookDecision::Allow
            }
            Err(_) => {
                tracing::error!("Prompt hook timed out");
                HookDecision::Block {
                    reason: "Prompt hook timed out".to_string(),
                }
            }
        }
    }
}

/// Agent handler: calls HookEvaluator for multi-turn evaluation with tool access.
struct AgentHandler {
    prompt_template: String,
    evaluator: Arc<dyn HookEvaluator>,
    timeout: std::time::Duration,
}

#[async_trait::async_trait]
impl HookHandler for AgentHandler {
    async fn handle(&self, event: &HookEvent) -> HookDecision {
        let arguments_json = event.to_command_input().to_string();
        let prompt = self.prompt_template.replace("$ARGUMENTS", &arguments_json);

        let result = tokio::time::timeout(self.timeout, async {
            self.evaluator.evaluate(&prompt, true).await
        })
        .await;

        match result {
            Ok(Ok(response_json)) => {
                match serde_json::from_str::<PromptHookResponse>(&response_json) {
                    Ok(response) => interpret_prompt_response(&response, event.kind()),
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Failed to parse agent hook response, treating as Allow"
                        );
                        HookDecision::Allow
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "Agent hook evaluator failed");
                HookDecision::Allow
            }
            Err(_) => {
                tracing::error!("Agent hook timed out");
                HookDecision::Block {
                    reason: "Agent hook timed out".to_string(),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Output interpretation
// ---------------------------------------------------------------------------

/// Interpret command hook JSON output based on event type.
///
/// Maps HookOutput fields to HookDecision following Claude Code semantics.
fn interpret_output(output: &HookOutput, event_kind: HookEventKind) -> HookDecision {
    // `continue: false` takes precedence over everything
    if !output.should_continue {
        return HookDecision::Cancel {
            reason: output
                .stop_reason
                .clone()
                .unwrap_or_else(|| "Hook requested stop".to_string()),
        };
    }

    // Check hookSpecificOutput
    if let Some(specific) = &output.hook_specific_output {
        if let Some(decision) = interpret_specific_output(specific) {
            return decision;
        }
    }

    // Top-level decision: "block"
    if let Some(decision) = &output.decision {
        if decision == &HookDecisionValue::Block {
            let reason = output
                .reason
                .clone()
                .unwrap_or_else(|| "Blocked by hook".to_string());
            // For Stop event, "block" means "don't stop" → ShouldContinue
            if event_kind == HookEventKind::Stop {
                return HookDecision::ShouldContinue { reason };
            }
            return HookDecision::Block { reason };
        }
    }

    // Additional context (top-level or in hookSpecificOutput)
    let context = output
        .additional_context
        .clone()
        .or_else(|| extract_specific_context(&output.hook_specific_output));

    if let Some(ctx) = context {
        return HookDecision::AllowWithContext { context: ctx };
    }

    HookDecision::Allow
}

/// Interpret hookSpecificOutput for PreToolUse events.
///
/// Returns `Some(decision)` if the specific output determines the outcome,
/// `None` to fall through to top-level fields.
fn interpret_specific_output(specific: &HookSpecificOutput) -> Option<HookDecision> {
    match specific {
        HookSpecificOutput::PreToolUse {
            permission_decision,
            permission_decision_reason,
            updated_input,
            additional_context,
        } => interpret_pre_tool_use_specific(
            permission_decision.as_deref(),
            permission_decision_reason,
            updated_input,
            additional_context,
        ),
        HookSpecificOutput::PostToolUse { additional_context }
        | HookSpecificOutput::PostToolUseFailure { additional_context }
        | HookSpecificOutput::UserPromptSubmit { additional_context }
        | HookSpecificOutput::SessionStart { additional_context }
        | HookSpecificOutput::Notification { additional_context } => additional_context
            .as_ref()
            .map(|ctx| HookDecision::AllowWithContext {
                context: ctx.clone(),
            }),
        HookSpecificOutput::Stop { reason } => reason
            .as_ref()
            .map(|r| HookDecision::ShouldContinue { reason: r.clone() }),
    }
}

/// Interpret PreToolUse-specific fields into a decision.
fn interpret_pre_tool_use_specific(
    permission_decision: Option<&str>,
    permission_decision_reason: &Option<String>,
    updated_input: &Option<serde_json::Value>,
    additional_context: &Option<String>,
) -> Option<HookDecision> {
    if let Some(decision) = permission_decision {
        match decision {
            "deny" | "block" => {
                return Some(HookDecision::Block {
                    reason: permission_decision_reason
                        .clone()
                        .unwrap_or_else(|| "Denied by hook".to_string()),
                });
            }
            "allow" => {
                if let Some(ctx) = additional_context {
                    return Some(HookDecision::AllowWithContext {
                        context: ctx.clone(),
                    });
                }
                return Some(HookDecision::Allow);
            }
            _ => {} // "ask" or unknown — fall through
        }
    }
    if let Some(input) = updated_input {
        return Some(HookDecision::AllowWithUpdatedInput {
            updated_input: input.clone(),
        });
    }
    if let Some(ctx) = additional_context {
        return Some(HookDecision::AllowWithContext {
            context: ctx.clone(),
        });
    }
    None
}

/// Extract additionalContext from a HookSpecificOutput if present.
fn extract_specific_context(specific: &Option<HookSpecificOutput>) -> Option<String> {
    match specific.as_ref()? {
        HookSpecificOutput::PreToolUse {
            additional_context, ..
        }
        | HookSpecificOutput::PostToolUse { additional_context }
        | HookSpecificOutput::PostToolUseFailure { additional_context }
        | HookSpecificOutput::UserPromptSubmit { additional_context }
        | HookSpecificOutput::SessionStart { additional_context }
        | HookSpecificOutput::Notification { additional_context } => additional_context.clone(),
        HookSpecificOutput::Stop { .. } => None,
    }
}

/// Interpret prompt/agent evaluator response based on event type.
fn interpret_prompt_response(
    response: &PromptHookResponse,
    event_kind: HookEventKind,
) -> HookDecision {
    if response.ok {
        HookDecision::Allow
    } else {
        let reason = response
            .reason
            .clone()
            .unwrap_or_else(|| "Blocked by prompt hook".to_string());
        if is_blockable(event_kind) {
            HookDecision::Block { reason }
        } else if event_kind == HookEventKind::Stop {
            HookDecision::ShouldContinue { reason }
        } else if feeds_stderr_to_agent(event_kind) {
            HookDecision::AllowWithContext { context: reason }
        } else {
            HookDecision::Allow
        }
    }
}

// ---------------------------------------------------------------------------
// Factory: config → registrations
// ---------------------------------------------------------------------------

/// Build a handler from config, requiring an evaluator for prompt/agent types.
fn build_handler(
    config: &HookHandlerConfig,
    evaluator: &Option<Arc<dyn HookEvaluator>>,
) -> Result<Arc<dyn HookHandler>, HookConfigError> {
    match config {
        HookHandlerConfig::Command { command, timeout } => Ok(Arc::new(CommandHandler {
            command: command.clone(),
            timeout: std::time::Duration::from_secs(*timeout),
        })),
        HookHandlerConfig::Prompt {
            prompt, timeout, ..
        } => {
            let eval = evaluator
                .as_ref()
                .ok_or(HookConfigError::MissingEvaluator)?
                .clone();
            Ok(Arc::new(PromptHandler {
                prompt_template: prompt.clone(),
                evaluator: eval,
                timeout: std::time::Duration::from_secs(*timeout),
            }))
        }
        HookHandlerConfig::Agent {
            prompt, timeout, ..
        } => {
            let eval = evaluator
                .as_ref()
                .ok_or(HookConfigError::MissingEvaluator)?
                .clone();
            Ok(Arc::new(AgentHandler {
                prompt_template: prompt.clone(),
                evaluator: eval,
                timeout: std::time::Duration::from_secs(*timeout),
            }))
        }
    }
}

impl HookConfig {
    /// Build runtime [`HookRegistration`]s from this config.
    ///
    /// Each matcher group + handler combination becomes one `HookRegistration`.
    /// Prompt/agent handlers require an evaluator.
    pub fn build_registrations(
        &self,
        evaluator: Option<Arc<dyn HookEvaluator>>,
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

                let matcher = group
                    .matcher
                    .as_deref()
                    .filter(|m| !m.is_empty() && *m != "*")
                    .map(regex::Regex::new)
                    .transpose()?;

                for handler_config in &group.hooks {
                    let handler = build_handler(handler_config, &evaluator)?;
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

/// Convenience: build a [`HookableAgent`] from config and an inner agent.
pub fn hookable_agent_from_config(
    inner: Arc<dyn agent_client_protocol::Agent + Send + Sync>,
    config: &HookConfig,
    evaluator: Option<Arc<dyn HookEvaluator>>,
) -> Result<crate::HookableAgent, HookConfigError> {
    let registrations = config.build_registrations(evaluator)?;
    let mut agent = crate::HookableAgent::new(inner);
    for reg in registrations {
        agent = agent.with_registration(reg);
    }
    Ok(agent)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{
        Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock,
        ExtNotification, ExtRequest, ExtResponse, InitializeRequest, InitializeResponse,
        LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
        PromptRequest, PromptResponse, SessionId, SetSessionModeRequest, SetSessionModeResponse,
        StopReason, TextContent,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    // -- Mock agent --

    struct MockAgent {
        prompt_called: Arc<AtomicBool>,
    }

    impl MockAgent {
        fn new() -> (Self, Arc<AtomicBool>) {
            let called = Arc::new(AtomicBool::new(false));
            (
                Self {
                    prompt_called: called.clone(),
                },
                called,
            )
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Agent for MockAgent {
        async fn initialize(
            &self,
            _req: InitializeRequest,
        ) -> agent_client_protocol::Result<InitializeResponse> {
            Ok(InitializeResponse::new(
                agent_client_protocol::ProtocolVersion::LATEST,
            ))
        }
        async fn authenticate(
            &self,
            _req: AuthenticateRequest,
        ) -> agent_client_protocol::Result<AuthenticateResponse> {
            Ok(AuthenticateResponse::new())
        }
        async fn new_session(
            &self,
            _req: NewSessionRequest,
        ) -> agent_client_protocol::Result<NewSessionResponse> {
            Ok(NewSessionResponse::new("test-session"))
        }
        async fn prompt(
            &self,
            _req: PromptRequest,
        ) -> agent_client_protocol::Result<PromptResponse> {
            self.prompt_called.store(true, Ordering::SeqCst);
            Ok(PromptResponse::new(StopReason::EndTurn))
        }
        async fn cancel(&self, _req: CancelNotification) -> agent_client_protocol::Result<()> {
            Ok(())
        }
        async fn load_session(
            &self,
            _req: LoadSessionRequest,
        ) -> agent_client_protocol::Result<LoadSessionResponse> {
            Ok(LoadSessionResponse::new())
        }
        async fn set_session_mode(
            &self,
            _req: SetSessionModeRequest,
        ) -> agent_client_protocol::Result<SetSessionModeResponse> {
            Ok(SetSessionModeResponse::new())
        }
        async fn ext_method(&self, _req: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
            Err(agent_client_protocol::Error::method_not_found())
        }
        async fn ext_notification(
            &self,
            _notif: ExtNotification,
        ) -> agent_client_protocol::Result<()> {
            Ok(())
        }
    }

    fn make_prompt_request() -> PromptRequest {
        PromptRequest::new(
            SessionId::from("test-session"),
            vec![ContentBlock::Text(TextContent::new("hello"))],
        )
    }

    // -- Mock evaluator --

    struct MockEvaluator {
        response: String,
        is_agent_called: Arc<AtomicBool>,
    }

    impl MockEvaluator {
        fn allowing() -> Self {
            Self {
                response: r#"{"ok": true}"#.to_string(),
                is_agent_called: Arc::new(AtomicBool::new(false)),
            }
        }

        fn blocking(reason: &str) -> Self {
            Self {
                response: format!(r#"{{"ok": false, "reason": "{}"}}"#, reason),
                is_agent_called: Arc::new(AtomicBool::new(false)),
            }
        }

        fn with_agent_tracking() -> (Self, Arc<AtomicBool>) {
            let flag = Arc::new(AtomicBool::new(false));
            (
                Self {
                    response: r#"{"ok": true}"#.to_string(),
                    is_agent_called: flag.clone(),
                },
                flag,
            )
        }
    }

    #[async_trait::async_trait]
    impl HookEvaluator for MockEvaluator {
        async fn evaluate(&self, _prompt: &str, is_agent: bool) -> Result<String, String> {
            if is_agent {
                self.is_agent_called.store(true, Ordering::SeqCst);
            }
            Ok(self.response.clone())
        }
    }

    // =====================================================================
    // JSON deserialization tests (3-level nesting)
    // =====================================================================

    #[test]
    fn test_json_command_hook() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "./check.sh"
                            }
                        ]
                    }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hooks.len(), 1);
        let groups = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(groups[0].hooks.len(), 1);
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Command { command, .. } if command == "./check.sh"
        ));
    }

    #[test]
    fn test_json_prompt_hook() {
        let json = r#"{
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            {
                                "type": "prompt",
                                "prompt": "Check if all tasks are complete: $ARGUMENTS"
                            }
                        ]
                    }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
        assert!(groups[0].matcher.is_none());
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Prompt { prompt, .. }
                if prompt.contains("$ARGUMENTS")
        ));
    }

    #[test]
    fn test_json_agent_hook() {
        let json = r#"{
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            {
                                "type": "agent",
                                "prompt": "Verify tests pass: $ARGUMENTS",
                                "timeout": 120
                            }
                        ]
                    }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Agent { prompt, timeout, .. }
                if prompt.contains("Verify tests") && *timeout == 120
        ));
    }

    #[test]
    fn test_json_multiple_events_with_matchers() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "./bash-check.sh" }
                        ]
                    },
                    {
                        "matcher": "Edit|Write",
                        "hooks": [
                            { "type": "command", "command": "./lint.sh" }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "hooks": [
                            { "type": "prompt", "prompt": "All done? $ARGUMENTS" }
                        ]
                    }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hooks.len(), 2);
        let pre_tool = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
        assert_eq!(pre_tool.len(), 2);
        assert_eq!(pre_tool[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(pre_tool[1].matcher.as_deref(), Some("Edit|Write"));
    }

    #[test]
    fn test_json_empty_config() {
        let json = "{}";
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert!(config.hooks.is_empty());
    }

    #[test]
    fn test_json_default_timeouts() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "hooks": [
                        { "type": "command", "command": "true" },
                        { "type": "prompt", "prompt": "check" },
                        { "type": "agent", "prompt": "verify" }
                    ]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let hooks = &config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap()[0].hooks;
        assert!(matches!(&hooks[0], HookHandlerConfig::Command { timeout, .. } if *timeout == 600));
        assert!(matches!(&hooks[1], HookHandlerConfig::Prompt { timeout, .. } if *timeout == 30));
        assert!(matches!(&hooks[2], HookHandlerConfig::Agent { timeout, .. } if *timeout == 60));
    }

    // =====================================================================
    // YAML deserialization tests
    // =====================================================================

    #[test]
    fn test_yaml_command_hook() {
        let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
"#;
        let config: HookConfig = serde_yaml::from_str(yaml).unwrap();
        let groups = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
        assert_eq!(groups[0].matcher.as_deref(), Some("Bash"));
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Command { command, .. } if command == "./check.sh"
        ));
    }

    #[test]
    fn test_yaml_prompt_hook() {
        let yaml = r#"
hooks:
  Stop:
    - hooks:
        - type: prompt
          prompt: "Check completion: $ARGUMENTS"
"#;
        let config: HookConfig = serde_yaml::from_str(yaml).unwrap();
        let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Prompt { prompt, .. } if prompt.contains("$ARGUMENTS")
        ));
    }

    #[test]
    fn test_yaml_multiple_events() {
        let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
  Stop:
    - hooks:
        - type: prompt
          prompt: "Verify completion"
  SessionStart:
    - matcher: "startup"
      hooks:
        - type: command
          command: "./init.sh"
"#;
        let config: HookConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.hooks.len(), 3);
    }

    #[test]
    fn test_yaml_empty_config() {
        let yaml = "{}";
        let config: HookConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.hooks.is_empty());
    }

    // =====================================================================
    // JSON ↔ YAML equivalence
    // =====================================================================

    #[test]
    fn test_json_yaml_equivalence() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "./check.sh" }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "hooks": [
                            { "type": "prompt", "prompt": "Done?" }
                        ]
                    }
                ]
            }
        }"#;

        let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
  Stop:
    - hooks:
        - type: prompt
          prompt: "Done?"
"#;

        let from_json: HookConfig = serde_json::from_str(json).unwrap();
        let from_yaml: HookConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(from_json.hooks.len(), from_yaml.hooks.len());
        // Both should have PreToolUse and Stop
        assert!(from_json
            .hooks
            .contains_key(&HookEventKindConfig::PreToolUse));
        assert!(from_yaml
            .hooks
            .contains_key(&HookEventKindConfig::PreToolUse));
        assert!(from_json.hooks.contains_key(&HookEventKindConfig::Stop));
        assert!(from_yaml.hooks.contains_key(&HookEventKindConfig::Stop));
    }

    // =====================================================================
    // Build registration tests
    // =====================================================================

    #[test]
    fn test_build_registrations_command() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "true" }
                        ]
                    }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let regs = config.build_registrations(None).unwrap();
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].events, vec![HookEventKind::PreToolUse]);
        assert!(regs[0].matcher.is_some());
    }

    #[test]
    fn test_build_registrations_invalid_regex() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "matcher": "[invalid",
                    "hooks": [{ "type": "command", "command": "true" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let result = config.build_registrations(None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HookConfigError::InvalidRegex(_)
        ));
    }

    #[test]
    fn test_build_registrations_missing_evaluator() {
        let json = r#"{
            "hooks": {
                "Stop": [{
                    "hooks": [{ "type": "prompt", "prompt": "check" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let result = config.build_registrations(None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HookConfigError::MissingEvaluator
        ));
    }

    #[test]
    fn test_build_registrations_prompt_with_evaluator() {
        let json = r#"{
            "hooks": {
                "Stop": [{
                    "hooks": [{ "type": "prompt", "prompt": "check" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let evaluator: Arc<dyn HookEvaluator> = Arc::new(MockEvaluator::allowing());
        let regs = config.build_registrations(Some(evaluator)).unwrap();
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].events, vec![HookEventKind::Stop]);
    }

    #[test]
    fn test_build_registrations_wildcard_matcher_treated_as_none() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "matcher": "*",
                    "hooks": [{ "type": "command", "command": "true" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let regs = config.build_registrations(None).unwrap();
        assert!(regs[0].matcher.is_none());
    }

    // =====================================================================
    // HookOutput interpretation tests
    // =====================================================================

    #[test]
    fn test_interpret_output_continue_false() {
        let output = HookOutput {
            should_continue: false,
            stop_reason: Some("Build failed".into()),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(
            decision,
            HookDecision::Cancel { reason } if reason == "Build failed"
        ));
    }

    #[test]
    fn test_interpret_output_pre_tool_use_deny() {
        let output = HookOutput {
            hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                permission_decision: Some("deny".into()),
                permission_decision_reason: Some("Not allowed".into()),
                updated_input: None,
                additional_context: None,
            }),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(
            decision,
            HookDecision::Block { reason } if reason == "Not allowed"
        ));
    }

    #[test]
    fn test_interpret_output_pre_tool_use_allow() {
        let output = HookOutput {
            hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                permission_decision: Some("allow".into()),
                permission_decision_reason: None,
                updated_input: None,
                additional_context: None,
            }),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(decision, HookDecision::Allow));
    }

    #[test]
    fn test_interpret_output_stop_block_is_should_continue() {
        let output = HookOutput {
            decision: Some(HookDecisionValue::Block),
            reason: Some("Tests not passing".into()),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::Stop);
        assert!(matches!(
            decision,
            HookDecision::ShouldContinue { reason } if reason == "Tests not passing"
        ));
    }

    #[test]
    fn test_interpret_output_user_prompt_block() {
        let output = HookOutput {
            decision: Some(HookDecisionValue::Block),
            reason: Some("Prompt rejected".into()),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::UserPromptSubmit);
        assert!(matches!(
            decision,
            HookDecision::Block { reason } if reason == "Prompt rejected"
        ));
    }

    #[test]
    fn test_interpret_output_additional_context() {
        let output = HookOutput {
            additional_context: Some("Extra info".into()),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::SessionStart);
        assert!(matches!(
            decision,
            HookDecision::AllowWithContext { context } if context == "Extra info"
        ));
    }

    #[test]
    fn test_interpret_output_empty_is_allow() {
        let output = HookOutput::default();
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(decision, HookDecision::Allow));
    }

    // =====================================================================
    // Prompt/agent response interpretation
    // =====================================================================

    #[test]
    fn test_prompt_response_ok_true() {
        let response = PromptHookResponse {
            ok: true,
            reason: None,
        };
        let decision = interpret_prompt_response(&response, HookEventKind::PreToolUse);
        assert!(matches!(decision, HookDecision::Allow));
    }

    #[test]
    fn test_prompt_response_ok_false_blocks() {
        let response = PromptHookResponse {
            ok: false,
            reason: Some("Forbidden".into()),
        };
        let decision = interpret_prompt_response(&response, HookEventKind::PreToolUse);
        assert!(matches!(
            decision,
            HookDecision::Block { reason } if reason == "Forbidden"
        ));
    }

    #[test]
    fn test_prompt_response_ok_false_stop_is_should_continue() {
        let response = PromptHookResponse {
            ok: false,
            reason: Some("Tests not complete".into()),
        };
        let decision = interpret_prompt_response(&response, HookEventKind::Stop);
        assert!(matches!(
            decision,
            HookDecision::ShouldContinue { reason } if reason == "Tests not complete"
        ));
    }

    #[test]
    fn test_prompt_response_ok_false_post_tool_feeds_context() {
        // For PostToolUse, ok=false should feed the reason back as context
        // rather than block — the tool already executed.
        let response = PromptHookResponse {
            ok: false,
            reason: Some("Lint warning detected".into()),
        };
        let decision = interpret_prompt_response(&response, HookEventKind::PostToolUse);
        assert!(matches!(
            decision,
            HookDecision::AllowWithContext { context } if context == "Lint warning detected"
        ));
    }

    #[test]
    fn test_prompt_response_ok_false_post_tool_failure_feeds_context() {
        // For PostToolUseFailure, ok=false should also feed context —
        // the tool already failed, can't block retroactively.
        let response = PromptHookResponse {
            ok: false,
            reason: Some("Failure noted".into()),
        };
        let decision = interpret_prompt_response(&response, HookEventKind::PostToolUseFailure);
        assert!(matches!(
            decision,
            HookDecision::AllowWithContext { context } if context == "Failure noted"
        ));
    }

    // =====================================================================
    // Integration: command hooks → HookableAgent → behavior
    // =====================================================================

    #[tokio::test]
    async fn test_command_hook_exit_0_allows() {
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "true" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let (mock, called) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, None).unwrap();

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_ok());
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_command_hook_exit_2_blocks() {
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "echo 'forbidden' >&2; exit 2" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let (mock, called) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, None).unwrap();

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
        assert!(result.unwrap_err().message.contains("forbidden"));
    }

    #[tokio::test]
    async fn test_command_hook_other_exit_allows() {
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "command", "command": "exit 1" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let (mock, called) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, None).unwrap();

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_ok());
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_command_hook_sends_json_stdin() {
        // Command that reads stdin and checks for expected fields
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": "input=$(cat); echo $input | python3 -c \"import sys,json; d=json.load(sys.stdin); assert d['hook_event_name']=='UserPromptSubmit'; assert 'session_id' in d\""
                    }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let (mock, called) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, None).unwrap();

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_ok());
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_command_hook_json_output_block() {
        // Command that outputs JSON with decision: "block"
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo '{\"decision\": \"block\", \"reason\": \"JSON blocked\"}'"
                    }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let (mock, called) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, None).unwrap();

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
        assert!(result.unwrap_err().message.contains("JSON blocked"));
    }

    #[tokio::test]
    async fn test_stop_exit_2_is_should_continue() {
        let json = r#"{
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo 'Tests not passing' >&2; exit 2"
                    }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let (mock, _) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, None).unwrap();

        let response = agent.prompt(make_prompt_request()).await.unwrap();
        let meta = response.meta.as_ref().unwrap();
        assert_eq!(
            meta.get("hook_should_continue"),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            meta.get("hook_reason"),
            Some(&serde_json::Value::String("Tests not passing".into()))
        );
    }

    // -- Event-aware exit-2 tests --

    #[test]
    fn test_exit_2_on_silent_events_allows() {
        // Notification and SessionStart: exit-2 stderr is shown to user
        // only (logged), not fed back to the agent.
        let silent = vec![HookEventKind::Notification, HookEventKind::SessionStart];

        for kind in &silent {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg("echo 'should not block' >&2; exit 2")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .unwrap();

            let decision = interpret_exit_2_stderr(&output, "test-cmd", *kind);
            assert!(
                matches!(decision, HookDecision::Allow),
                "Expected Allow for silent {:?}, got {:?}",
                kind,
                decision
            );
        }
    }

    #[tokio::test]
    async fn test_post_tool_use_exit_2_feeds_stderr_as_context() {
        // PostToolUse hooks that exit with code 2 should feed stderr
        // back to the agent as context (AllowWithContext), not block.
        use crate::hookable_agent::HookEvent;
        use std::path::PathBuf;

        let json = r#"{
            "hooks": {
                "PostToolUse": [{
                    "hooks": [{ "type": "command", "command": "echo 'tool feedback' >&2; exit 2" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let regs = config.build_registrations(None).unwrap();
        let event = HookEvent::PostToolUse {
            session_id: "s1".into(),
            tool_name: "Bash".into(),
            tool_input: None,
            tool_response: None,
            tool_use_id: None,
            cwd: PathBuf::from("/tmp"),
        };
        let decision = regs[0].handler.handle(&event).await;
        assert!(
            matches!(decision, HookDecision::AllowWithContext { ref context } if context == "tool feedback"),
            "Expected AllowWithContext, got {:?}",
            decision
        );
    }

    #[tokio::test]
    async fn test_post_tool_use_failure_exit_2_feeds_stderr_as_context() {
        // PostToolUseFailure hooks that exit with code 2 should also feed
        // stderr back as context — the tool already failed, can't block.
        use crate::hookable_agent::HookEvent;
        use std::path::PathBuf;

        let json = r#"{
            "hooks": {
                "PostToolUseFailure": [{
                    "hooks": [{ "type": "command", "command": "echo 'failure feedback' >&2; exit 2" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let regs = config.build_registrations(None).unwrap();
        let event = HookEvent::PostToolUseFailure {
            session_id: "s1".into(),
            tool_name: "Bash".into(),
            tool_input: None,
            error: None,
            tool_use_id: None,
            cwd: PathBuf::from("/tmp"),
        };
        let decision = regs[0].handler.handle(&event).await;
        assert!(
            matches!(decision, HookDecision::AllowWithContext { ref context } if context == "failure feedback"),
            "Expected AllowWithContext, got {:?}",
            decision
        );
    }

    #[test]
    fn test_exit_2_on_blockable_event_blocks() {
        // Only PreToolUse and UserPromptSubmit can block — the action
        // hasn't happened yet. PostToolUseFailure cannot block because
        // the tool already failed.
        let blockable = vec![HookEventKind::PreToolUse, HookEventKind::UserPromptSubmit];

        for kind in &blockable {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg("echo 'blocked' >&2; exit 2")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .unwrap();

            let decision = interpret_exit_2_stderr(&output, "test-cmd", *kind);
            assert!(
                matches!(decision, HookDecision::Block { .. }),
                "Expected Block for blockable {:?}, got {:?}",
                kind,
                decision
            );
        }
    }

    // =====================================================================
    // Integration: prompt/agent hooks
    // =====================================================================

    #[tokio::test]
    async fn test_prompt_hook_ok_true_allows() {
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "prompt", "prompt": "Check: $ARGUMENTS" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let evaluator: Arc<dyn HookEvaluator> = Arc::new(MockEvaluator::allowing());
        let (mock, called) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, Some(evaluator)).unwrap();

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_ok());
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_prompt_hook_ok_false_blocks() {
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "prompt", "prompt": "Check: $ARGUMENTS" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let evaluator: Arc<dyn HookEvaluator> = Arc::new(MockEvaluator::blocking("Not allowed"));
        let (mock, called) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, Some(evaluator)).unwrap();

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
        assert!(result.unwrap_err().message.contains("Not allowed"));
    }

    #[tokio::test]
    async fn test_agent_hook_calls_with_is_agent_true() {
        let json = r#"{
            "hooks": {
                "UserPromptSubmit": [{
                    "hooks": [{ "type": "agent", "prompt": "Verify: $ARGUMENTS" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let (eval, is_agent_flag) = MockEvaluator::with_agent_tracking();
        let evaluator: Arc<dyn HookEvaluator> = Arc::new(eval);
        let (mock, _) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, Some(evaluator)).unwrap();

        let _ = agent.prompt(make_prompt_request()).await.unwrap();
        assert!(is_agent_flag.load(Ordering::SeqCst));
    }

    // =====================================================================
    // Full lifecycle test
    // =====================================================================

    #[tokio::test]
    async fn test_full_lifecycle_from_json_config() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "true", "timeout": 10 }
                        ]
                    }
                ],
                "UserPromptSubmit": [
                    {
                        "hooks": [
                            { "type": "command", "command": "true" }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "echo '{\"decision\": \"block\", \"reason\": \"Keep going\"}'"
                            }
                        ]
                    }
                ]
            }
        }"#;

        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hooks.len(), 3);

        let registrations = config.build_registrations(None).unwrap();
        assert_eq!(registrations.len(), 3);

        let (mock, _) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, None).unwrap();

        let response = agent.prompt(make_prompt_request()).await.unwrap();
        let meta = response.meta.as_ref().unwrap();
        assert_eq!(
            meta.get("hook_should_continue"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    #[tokio::test]
    async fn test_full_lifecycle_from_yaml_config() {
        let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "true"
          timeout: 10
  UserPromptSubmit:
    - hooks:
        - type: command
          command: "true"
  Stop:
    - hooks:
        - type: command
          command: "echo '{\"decision\": \"block\", \"reason\": \"Keep going\"}'"
"#;

        let config: HookConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.hooks.len(), 3);

        let (mock, _) = MockAgent::new();
        let agent = hookable_agent_from_config(Arc::new(mock), &config, None).unwrap();

        let response = agent.prompt(make_prompt_request()).await.unwrap();
        let meta = response.meta.as_ref().unwrap();
        assert_eq!(
            meta.get("hook_should_continue"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    // =====================================================================
    // Forward-compatible event kinds
    // =====================================================================

    #[test]
    fn test_unsupported_event_kinds_return_error() {
        let unsupported_kinds = vec![
            HookEventKindConfig::PermissionRequest,
            HookEventKindConfig::SubagentStart,
            HookEventKindConfig::SubagentStop,
            HookEventKindConfig::PreCompact,
            HookEventKindConfig::Setup,
            HookEventKindConfig::SessionEnd,
            HookEventKindConfig::TeammateIdle,
            HookEventKindConfig::TaskCompleted,
        ];

        for kind in &unsupported_kinds {
            let result: Result<HookEventKind, _> = kind.clone().try_into();
            assert!(result.is_err(), "Expected {:?} to be unsupported", kind);
        }
    }

    #[test]
    fn test_supported_event_kinds_succeed() {
        let supported_kinds = vec![
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
        ];

        for (config_kind, expected_kind) in &supported_kinds {
            let result: Result<HookEventKind, _> = config_kind.clone().try_into();
            assert_eq!(
                result.unwrap(),
                *expected_kind,
                "Expected {:?} to convert successfully",
                config_kind
            );
        }
    }

    #[test]
    fn test_unsupported_event_kind_display() {
        let err = UnsupportedEventKind;
        assert_eq!(err.to_string(), "event kind is not supported by ACP");
    }

    #[test]
    fn test_unsupported_event_kind_is_error() {
        let err = UnsupportedEventKind;
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_post_tool_use_failure_deserialization() {
        let json = r#"{
            "hooks": {
                "PostToolUseFailure": [{
                    "hooks": [{ "type": "command", "command": "echo 'Tool failed'" }]
                }]
            }
        }"#;

        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hooks.len(), 1);
        assert!(config
            .hooks
            .contains_key(&HookEventKindConfig::PostToolUseFailure));
    }

    // =====================================================================
    // HookDecisionValue enum
    // =====================================================================

    #[test]
    fn test_hook_decision_value_serialization() {
        assert_eq!(
            serde_json::to_string(&HookDecisionValue::Allow).unwrap(),
            "\"allow\""
        );
        assert_eq!(
            serde_json::to_string(&HookDecisionValue::Block).unwrap(),
            "\"block\""
        );
        assert_eq!(
            serde_json::to_string(&HookDecisionValue::Ask).unwrap(),
            "\"ask\""
        );
    }

    #[test]
    fn test_hook_decision_value_deserialization() {
        assert_eq!(
            serde_json::from_str::<HookDecisionValue>("\"allow\"").unwrap(),
            HookDecisionValue::Allow
        );
        assert_eq!(
            serde_json::from_str::<HookDecisionValue>("\"block\"").unwrap(),
            HookDecisionValue::Block
        );
        assert_eq!(
            serde_json::from_str::<HookDecisionValue>("\"ask\"").unwrap(),
            HookDecisionValue::Ask
        );
    }

    #[test]
    fn test_hook_output_with_decision_value() {
        let json = r#"{
            "continue": true,
            "decision": "block",
            "reason": "Blocked by hook"
        }"#;

        let output: HookOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.decision, Some(HookDecisionValue::Block));
        assert_eq!(output.reason, Some("Blocked by hook".to_string()));
    }

    #[test]
    fn test_pre_tool_use_deny_decision_parses_with_reason() {
        let json = r#"{
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "Too risky"
        }"#;

        let output: HookSpecificOutput = serde_json::from_str(json).unwrap();
        match output {
            HookSpecificOutput::PreToolUse {
                permission_decision,
                permission_decision_reason,
                ..
            } => {
                assert_eq!(permission_decision, Some("deny".to_string()));
                assert_eq!(permission_decision_reason, Some("Too risky".to_string()));
            }
            other => panic!("Expected PreToolUse, got {:?}", other),
        }
    }

    #[test]
    fn test_interpret_output_with_enum_block_decision() {
        let output = HookOutput {
            should_continue: true,
            stop_reason: None,
            suppress_output: false,
            system_message: None,
            decision: Some(HookDecisionValue::Block),
            reason: Some("Blocked".to_string()),
            hook_specific_output: None,
            additional_context: None,
        };

        let decision = interpret_output(&output, HookEventKind::UserPromptSubmit);
        assert!(matches!(decision, HookDecision::Block { .. }));
    }

    #[test]
    fn test_interpret_output_with_enum_permission_decision() {
        let output = HookOutput {
            hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                permission_decision: Some("deny".into()),
                permission_decision_reason: Some("Denied".into()),
                updated_input: None,
                additional_context: None,
            }),
            ..Default::default()
        };

        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(decision, HookDecision::Block { .. }));
    }
}
