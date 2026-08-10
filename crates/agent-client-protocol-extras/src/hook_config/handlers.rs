//! The two built-in hook handlers.
//!
//! [`CommandHandler`] runs a shell command: it sends the event JSON on stdin,
//! and reads the exit code and stdout back. [`EvaluatorHandler`] asks a
//! [`HookEvaluator`] instead, and backs both the `prompt` and the `agent` hook
//! type.
//!
//! Both handlers give the answer to [`interpret_output`] or
//! [`interpret_prompt_response`] in [`super::output`], so the rules that make a
//! [`HookDecision`] live in one place.

use super::decision::{HookDecision, HookEvaluator, HookHandler};
use super::event::{HookCommandContext, HookEvent, HookEventKind};
use super::output::{
    feeds_stderr_to_agent, interpret_output, interpret_prompt_response, is_blockable, HookOutput,
    PromptHookResponse,
};
use std::sync::Arc;

/// Command handler: runs shell command with JSON stdin/stdout protocol.
///
/// Exit codes (following Claude Code):
/// - 0 → parse stdout as HookOutput JSON, interpret based on event
/// - 2 → Block (stderr becomes reason)
/// - Other → Allow (warning logged)
///
/// `pub(super)` rather than private, because the factory that builds it from a
/// config lives in the sibling [`super::config`] file.
pub(super) struct CommandHandler {
    /// The shell command line to run.
    pub(super) command: String,
    /// How long the command may run before it counts as timed out.
    pub(super) timeout: std::time::Duration,
    /// AVP context fields (`transcript_path`, `permission_mode`) folded into the
    /// command's JSON stdin so a hook sees the same input shape Claude Code
    /// sends. Captured at build time from the caller's [`HookCommandContext`].
    pub(super) command_context: HookCommandContext,
}

#[async_trait::async_trait]
impl HookHandler for CommandHandler {
    async fn handle(&self, event: &HookEvent) -> HookDecision {
        let stdin_json = event
            .to_command_input_full(&self.command_context)
            .to_string();
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

/// Exit code meaning "parse stdout as HookOutput JSON, interpret based on event".
const EXIT_CODE_SUCCESS: i32 = 0;

/// Exit code meaning "Block (stderr becomes reason)".
const EXIT_CODE_BLOCK: i32 = 2;

/// Interpret a command's exit code into a HookDecision.
fn interpret_exit_code(
    output: &std::process::Output,
    command: &str,
    event_kind: HookEventKind,
) -> HookDecision {
    let code = output.status.code().unwrap_or(-1);
    match code {
        EXIT_CODE_SUCCESS => interpret_exit_0_stdout(output, command, event_kind),
        EXIT_CODE_BLOCK => interpret_exit_2_stderr(output, command, event_kind),
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

/// Parse a hook command's stdout into a [`HookOutput`].
///
/// JSON is the documented protocol, so it is tried first and its error is the
/// one reported. YAML is tried second because a hook command is any program the
/// user names, and several of ours print YAML for a human; JSON is a subset of
/// YAML, so accepting both costs nothing and loses no strictness. The same
/// try-JSON-then-YAML shape is used when the CLI reads piped tool arguments
/// (`merge_parsed_stdin` in the `sah` binary).
///
/// Before the fallback existed, YAML stdout failed the JSON parse and the hook's
/// decision was discarded as `Allow` with only a warning — the hook appeared to
/// run and did nothing.
fn parse_hook_stdout(stdout: &str) -> Result<HookOutput, serde_json::Error> {
    serde_json::from_str::<HookOutput>(stdout).or_else(|json_error| {
        serde_yaml_ng::from_str::<HookOutput>(stdout).map_err(|_yaml_error| json_error)
    })
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
    match parse_hook_stdout(stdout) {
        Ok(hook_output) => interpret_output(&hook_output, event_kind),
        Err(e) => {
            tracing::warn!(
                command = %command,
                error = %e,
                stdout = %stdout,
                "Failed to parse hook command output as JSON or YAML, treating as Allow"
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

/// Prompt/agent handler: calls a [`HookEvaluator`] for LLM-backed evaluation.
///
/// `is_agent` selects the evaluation mode passed through to
/// [`HookEvaluator::evaluate`] — single-turn (`type: prompt`, `is_agent=false`)
/// vs. multi-turn with tool access (`type: agent`, `is_agent=true`) — and also
/// labels log messages and the timeout's block reason, so the two hook types
/// share one implementation instead of two near-identical copies.
///
/// `pub(super)` rather than private, because the factory that builds it from a
/// config lives in the sibling [`super::config`] file.
pub(super) struct EvaluatorHandler {
    /// The prompt text, with `$ARGUMENTS` still in it. The handler replaces
    /// that placeholder with the event JSON before it asks the evaluator.
    pub(super) prompt_template: String,
    /// The evaluator that answers the prompt.
    pub(super) evaluator: Arc<dyn HookEvaluator>,
    /// How long the evaluator may take before it counts as timed out.
    pub(super) timeout: std::time::Duration,
    /// AVP context fields (`transcript_path`, `permission_mode`) folded into
    /// the event JSON the prompt carries, so a hook sees the same input shape
    /// Claude Code sends.
    pub(super) command_context: HookCommandContext,
    /// `true` for a `type: agent` hook (multi-turn, with tool access), `false`
    /// for a `type: prompt` hook (single turn). It also selects the label the
    /// log messages and the timeout reason use.
    pub(super) is_agent: bool,
}

impl EvaluatorHandler {
    /// Human-readable label for this handler's mode, used in log messages
    /// and timeout reasons (e.g. "Prompt hook timed out").
    fn label(&self) -> &'static str {
        if self.is_agent {
            "Agent"
        } else {
            "Prompt"
        }
    }
}

#[async_trait::async_trait]
impl HookHandler for EvaluatorHandler {
    async fn handle(&self, event: &HookEvent) -> HookDecision {
        let arguments_json = event
            .to_command_input_full(&self.command_context)
            .to_string();
        let prompt = self.prompt_template.replace("$ARGUMENTS", &arguments_json);
        let label = self.label();

        let result = tokio::time::timeout(self.timeout, async {
            self.evaluator.evaluate(&prompt, self.is_agent).await
        })
        .await;

        match result {
            Ok(Ok(response_json)) => {
                match serde_json::from_str::<PromptHookResponse>(&response_json) {
                    Ok(response) => interpret_prompt_response(&response, event.kind()),
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Failed to parse {label} hook response, treating as Allow"
                        );
                        HookDecision::Allow
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "{label} hook evaluator failed");
                HookDecision::Allow
            }
            Err(_) => {
                tracing::error!("{label} hook timed out");
                HookDecision::Block {
                    reason: format!("{label} hook timed out"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
