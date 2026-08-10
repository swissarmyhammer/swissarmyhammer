//! What a hook prints, and how that becomes a decision.
//!
//! The types here mirror Claude Code's hook output JSON: [`HookOutput`] is the
//! whole document a command hook prints on stdout, [`HookSpecificOutput`] is
//! its per-event part, and [`PromptHookResponse`] is the smaller answer a
//! `prompt` or `agent` hook gives.
//!
//! [`interpret_output`] and [`interpret_prompt_response`] turn one of those
//! answers, plus the kind of the event, into a [`HookDecision`]. The rules
//! depend on the event kind, because only some kinds can block, and the
//! `EVENT_PROPERTIES` table holds that fact once for every rule that reads
//! it.
//!
//! A hook can also refuse: a command hook exits with code 2, and a prompt
//! hook answers `ok: false`. [`decide_by_event_kind`] turns either refusal
//! into a decision, so both refusals obey one set of rules.

use super::decision::HookDecision;
use super::event::HookEventKind;
use serde::{Deserialize, Serialize};

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

/// Builder for [`HookOutput`]'s six optional fields.
///
/// `HookOutput` starts from [`HookOutput::default`] (`should_continue: true`,
/// `suppress_output: false`, every `Option` field `None`); each `with_*`
/// method sets one optional field and returns `Self` for chaining, and
/// [`HookOutputBuilder::build`] produces the finished value. This exists to
/// make optional-field construction explicit and readable in place of a
/// struct literal with `..Default::default()`.
#[derive(Clone, Debug, Default)]
pub struct HookOutputBuilder {
    output: HookOutput,
}

impl HookOutputBuilder {
    /// Start a new builder from `HookOutput::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `stop_reason`, the message shown when `should_continue` is false.
    pub fn with_stop_reason(mut self, stop_reason: impl Into<String>) -> Self {
        self.output.stop_reason = Some(stop_reason.into());
        self
    }

    /// Set `system_message`, a warning message shown to the user.
    pub fn with_system_message(mut self, system_message: impl Into<String>) -> Self {
        self.output.system_message = Some(system_message.into());
        self
    }

    /// Set `decision`, the top-level allow/block/ask decision.
    pub fn with_decision(mut self, decision: HookDecisionValue) -> Self {
        self.output.decision = Some(decision);
        self
    }

    /// Set `reason`, the explanation for `decision`.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.output.reason = Some(reason.into());
        self
    }

    /// Set `hook_specific_output`, the event-specific output payload.
    pub fn with_hook_specific_output(mut self, hook_specific_output: HookSpecificOutput) -> Self {
        self.output.hook_specific_output = Some(hook_specific_output);
        self
    }

    /// Set `additional_context`, a string appended to Claude's context.
    pub fn with_additional_context(mut self, additional_context: impl Into<String>) -> Self {
        self.output.additional_context = Some(additional_context.into());
        self
    }

    /// Consume the builder and produce the finished [`HookOutput`].
    pub fn build(self) -> HookOutput {
        self.output
    }
}

/// Event-specific output fields inside `hookSpecificOutput`.
///
/// Tagged by `hookEventName` to enforce per-event field sets, matching
/// AVP's `#[serde(tag = "hookEventName")]` convention.
///
/// A hook sets only the field(s) it cares about; every other field is
/// absent, not `null`. Every field on every variant is therefore
/// `#[serde(default)]` so a partial `hookSpecificOutput` — one field
/// present, the rest missing — deserializes with the missing fields as
/// `None` and its one present field still drives the decision, instead of
/// the whole `HookOutput` document failing to parse. serde's derive already
/// treats a missing `Option<T>` field as `None` without this attribute, but
/// it is written explicitly here so the contract does not depend on that
/// implicit behavior surviving a future edit (e.g. a field type that stops
/// being spelled literally as `Option<...>`).
///
/// A `hookSpecificOutput` that is genuinely unparseable — an unrecognized
/// `hookEventName`, or a value of the wrong shape — is a deliberate error,
/// not a silently permissive default: `serde_json::from_str::<HookOutput>`
/// (via [`parse_hook_stdout`](super::handlers::parse_hook_stdout)) fails, and
/// the caller (`interpret_exit_0_stdout`) logs that failure at
/// `tracing::warn!` — command, error, and raw stdout — before falling back to
/// `HookDecision::Allow`. `Allow` is the permissive direction, so the log is
/// the only signal that a malformed deny became a permit; it is deliberately
/// visible rather than silent.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "hookEventName")]
pub enum HookSpecificOutput {
    /// PreToolUse-specific output fields.
    PreToolUse {
        /// The JSON `permissionDecision` key. `"deny"` or `"block"` stops the
        /// tool call, `"allow"` lets it run, and `"ask"` or an unknown value
        /// goes on to the next field. Absent means the hook made no
        /// permission decision.
        #[serde(rename = "permissionDecision", default)]
        permission_decision: Option<String>,
        /// The JSON `permissionDecisionReason` key. It gives the text of a
        /// block. Absent means the block uses the default text.
        #[serde(rename = "permissionDecisionReason", default)]
        permission_decision_reason: Option<String>,
        /// The JSON `updatedInput` key. It holds new tool arguments. Absent
        /// means the hook does not change the arguments. ACP cannot apply a
        /// change to a call that already started, so the agent only writes it
        /// to the log.
        #[serde(rename = "updatedInput", default)]
        updated_input: Option<serde_json::Value>,
        /// The JSON `additionalContext` key. It holds text to add to the
        /// context of the agent. Absent means the hook adds no text.
        #[serde(rename = "additionalContext", default)]
        additional_context: Option<String>,
    },
    /// PostToolUse-specific output fields.
    PostToolUse {
        /// The JSON `additionalContext` key. It holds text to add to the
        /// context of the agent after the tool ran. Absent means the hook adds
        /// no text.
        #[serde(rename = "additionalContext", default)]
        additional_context: Option<String>,
    },
    /// PostToolUseFailure-specific output fields.
    PostToolUseFailure {
        /// The JSON `additionalContext` key. It holds text to add to the
        /// context of the agent after the tool failed. Absent means the hook
        /// adds no text.
        #[serde(rename = "additionalContext", default)]
        additional_context: Option<String>,
    },
    /// UserPromptSubmit-specific output fields.
    UserPromptSubmit {
        /// The JSON `additionalContext` key. It holds text to add in front of
        /// the user prompt. Absent means the hook adds no text.
        #[serde(rename = "additionalContext", default)]
        additional_context: Option<String>,
    },
    /// Stop-specific output fields.
    Stop {
        /// The JSON `reason` key. It tells the agent why it must not stop.
        /// Absent means the hook lets the agent stop.
        #[serde(default)]
        reason: Option<String>,
    },
    /// SessionStart-specific output fields.
    SessionStart {
        /// The JSON `additionalContext` key. It holds text to add to the
        /// context at the start of the session. Absent means the hook adds no
        /// text.
        #[serde(rename = "additionalContext", default)]
        additional_context: Option<String>,
    },
    /// Notification-specific output fields.
    Notification {
        /// The JSON `additionalContext` key. It holds text to add to the
        /// context when the notification occurs. Absent means the hook adds no
        /// text.
        #[serde(rename = "additionalContext", default)]
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

/// How one event kind handles a hook that exits with code 2.
///
/// The two properties travel together because they come from one row of
/// [`EVENT_PROPERTIES`], so one lookup reads both.
#[derive(Clone, Copy)]
struct EventProperties {
    /// Whether the event kind can block the action.
    blockable: bool,
    /// Whether the stderr of the hook goes to the agent as context.
    feeds_stderr_to_agent: bool,
}

impl EventProperties {
    /// The properties of an event kind that [`EVENT_PROPERTIES`] does not
    /// list: an informational event that can neither block nor give its
    /// stderr to the agent.
    const NONE: Self = Self {
        blockable: false,
        feeds_stderr_to_agent: false,
    };
}

/// Per-event-kind exit-2 handling properties.
///
/// Centralizes both properties in one table instead of two parallel match
/// statements over `HookEventKind`, so [`is_blockable`] and
/// [`feeds_stderr_to_agent`] cannot drift out of sync as event kinds are
/// added. A kind absent from this table gets [`EventProperties::NONE`].
const EVENT_PROPERTIES: &[(HookEventKind, EventProperties)] = &[
    (
        HookEventKind::PreToolUse,
        EventProperties {
            blockable: true,
            feeds_stderr_to_agent: false,
        },
    ),
    (
        HookEventKind::UserPromptSubmit,
        EventProperties {
            blockable: true,
            feeds_stderr_to_agent: false,
        },
    ),
    (
        HookEventKind::PostToolUse,
        EventProperties {
            blockable: false,
            feeds_stderr_to_agent: true,
        },
    ),
    (
        HookEventKind::PostToolUseFailure,
        EventProperties {
            blockable: false,
            feeds_stderr_to_agent: true,
        },
    ),
];

/// Read the exit-2 handling properties of one event kind.
///
/// This is the one lookup of [`EVENT_PROPERTIES`]. Each property has its own
/// reader below, and both readers come through here, so the table is searched
/// one way only.
fn event_properties(kind: HookEventKind) -> EventProperties {
    EVENT_PROPERTIES
        .iter()
        .find(|(table_kind, _)| *table_kind == kind)
        .map(|(_, properties)| *properties)
        .unwrap_or(EventProperties::NONE)
}

/// Whether an event kind supports blocking via exit-2.
///
/// Only PreToolUse and UserPromptSubmit can block because the action
/// hasn't happened yet. All other events (PostToolUse, PostToolUseFailure,
/// Notification, SessionStart) cannot block.
fn is_blockable(kind: HookEventKind) -> bool {
    event_properties(kind).blockable
}

/// Whether exit-2 stderr should be fed back as agent context.
///
/// PostToolUse and PostToolUseFailure can't block (action already happened)
/// but Claude Code feeds the stderr back to the agent as context.
fn feeds_stderr_to_agent(kind: HookEventKind) -> bool {
    event_properties(kind).feeds_stderr_to_agent
}

/// Turn the refusal of a hook into a decision, by the kind of the event.
///
/// A command hook refuses with exit code 2 and the text on its stderr, and a
/// prompt hook refuses with `ok: false` and its reason. Both refusals say the
/// same thing, so both come here and get the same four rules:
///
/// - an event kind that can still block gives [`HookDecision::Block`];
/// - a `Stop` event gives [`HookDecision::ShouldContinue`], because a refusal
///   to stop is an instruction to go on;
/// - an event kind that gives its stderr to the agent gives
///   [`HookDecision::AllowWithContext`];
/// - every other event kind gives [`HookDecision::Allow`], because the action
///   already happened and the agent never reads the reason.
///
/// The last rule loses the message of the hook. The caller logs that, because
/// only the caller knows what to name in the log.
pub(super) fn decide_by_event_kind(event_kind: HookEventKind, reason: String) -> HookDecision {
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

// ---------------------------------------------------------------------------
// Output interpretation
// ---------------------------------------------------------------------------

/// Interpret command hook JSON output based on event type.
///
/// Maps HookOutput fields to HookDecision following Claude Code semantics.
pub(super) fn interpret_output(output: &HookOutput, event_kind: HookEventKind) -> HookDecision {
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
///
/// `ok: true` allows. `ok: false` is a refusal, so
/// [`decide_by_event_kind`] gives the decision.
pub(super) fn interpret_prompt_response(
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
        decide_by_event_kind(event_kind, reason)
    }
}

#[cfg(test)]
mod tests;
