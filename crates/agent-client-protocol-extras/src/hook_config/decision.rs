//! What a hook answers, and who answers it.
//!
//! A hook handler reads a [`HookEvent`] and returns a [`HookDecision`]. Two
//! traits do the reading: [`HookHandler`] is the general one, and
//! [`HookEvaluator`] is the LLM-backed one that the `prompt` and `agent` hook
//! types use.
//!
//! This file also holds the matcher rules that decide WHICH events reach a
//! handler, and the [`HookRegistration`] that binds an event kind, a matcher,
//! and a handler together.

use super::event::{HookEvent, HookEventKind};
use std::sync::Arc;

/// What a hook handler wants to happen after it runs.
///
/// Always derived from handler output at runtime (command JSON, prompt/agent
/// evaluator response), never configured statically.
#[derive(Clone, Debug, Default)]
pub enum HookDecision {
    /// Allow the operation to proceed unchanged.
    #[default]
    Allow,
    /// Block the operation (returned as ACP error).
    Block {
        /// The text that tells the user why the hook stopped the operation.
        /// It becomes the message of the ACP error.
        reason: String,
    },
    /// Allow but inject additional context (prepend text to prompt).
    AllowWithContext {
        /// The text to put in front of the prompt.
        context: String,
    },
    /// Cancel the active prompt turn by calling inner.cancel().
    Cancel {
        /// The text that tells the user why the hook cancelled the turn.
        reason: String,
    },
    /// Signal that the agent should not have stopped.
    /// Response meta gets `hook_should_continue: true`.
    ShouldContinue {
        /// The text that tells the agent why it must continue.
        reason: String,
    },
    /// Allow but modify tool input before execution (PreToolUse only).
    /// Note: In ACP, PreToolUse fires from notifications after tool initiation,
    /// so updatedInput cannot actually modify the call. Logged and treated as Allow.
    AllowWithUpdatedInput {
        /// The new tool arguments the hook asks for. ACP cannot apply them to
        /// a call that already started, so the agent only writes them to the
        /// log and allows the call.
        updated_input: serde_json::Value,
    },
}

/// Async handler invoked when a matching hook event fires.
///
/// Uses `#[async_trait]` (Send) for tokio::spawn compatibility.
#[async_trait::async_trait]
pub trait HookHandler: Send + Sync {
    /// Inspect the event and return a decision.
    async fn handle(&self, event: &HookEvent) -> HookDecision;
}

// ---------------------------------------------------------------------------
// Matcher
// ---------------------------------------------------------------------------

/// How a hook matcher decides whether an event's matcher value applies.
///
/// Mirrors Claude Code's documented matcher rules so the same config behaves
/// identically across both runtimes:
///
/// - `"*"`, `""`, or omitted → [`Matcher::All`] (fires for every occurrence).
/// - A matcher containing only `[A-Za-z0-9_|]` → [`Matcher::Exact`]: a `|`-separated
///   list of exact tool names, compared for FULL-string equality (not substring).
///   This is what prevents `"Bash"` from matching `Bash2` or `xBash`.
/// - Anything else → [`Matcher::Regex`]: a JavaScript-style regex evaluated like
///   `RegExp(pattern).test(value)`. It is intentionally unanchored, so the
///   pattern author controls anchoring via `^`/`$`: `mcp__memory__.*` matches
///   `mcp__memory__create_entities`, and `^Notebook` matches `NotebookEdit`.
#[derive(Clone, Debug)]
pub enum Matcher {
    /// Matches every event value.
    All,
    /// Matches when the event value equals one of these exact names.
    Exact(Vec<String>),
    /// Matches when the regex matches anywhere in the event value (JS `test`).
    Regex(regex::Regex),
}

impl Matcher {
    /// Classify a raw matcher string into a [`Matcher`].
    ///
    /// Returns [`regex::Error`] only when the string is a regex matcher that
    /// fails to compile; exact and all matchers never error.
    pub fn try_parse(raw: &str) -> Result<Self, regex::Error> {
        if raw.is_empty() || raw == "*" {
            return Ok(Self::All);
        }
        if is_identifier_matcher(raw) {
            let names = raw.split('|').map(str::to_string).collect();
            return Ok(Self::Exact(names));
        }
        Ok(Self::Regex(regex::Regex::new(raw)?))
    }

    /// Does this matcher accept the given event matcher value?
    fn matches_value(&self, value: &str) -> bool {
        match self {
            Self::All => true,
            Self::Exact(names) => names.iter().any(|n| n == value),
            Self::Regex(re) => re.is_match(value),
        }
    }
}

/// Whether a matcher string is a plain identifier / alternation of identifiers
/// (`[A-Za-z0-9_|]` only), which Claude Code treats as exact strings rather
/// than as a regular expression.
fn is_identifier_matcher(raw: &str) -> bool {
    raw.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '|')
}

// ---------------------------------------------------------------------------
// Hook registration
// ---------------------------------------------------------------------------

/// A registered hook: event filter + matcher + handler.
pub struct HookRegistration {
    /// Which event kinds this hook fires on.
    pub events: Vec<HookEventKind>,
    /// Matcher to filter events by value (tool name, event source, etc.).
    pub matcher: Matcher,
    /// The handler to invoke when this hook matches an event.
    pub handler: Arc<dyn HookHandler>,
}

impl Clone for HookRegistration {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            matcher: self.matcher.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}

impl std::fmt::Debug for HookRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRegistration")
            .field("events", &self.events)
            .field("matcher", &self.matcher)
            .field("handler", &"<dyn HookHandler>")
            .finish()
    }
}

impl HookRegistration {
    /// Create a new hook registration.
    pub fn new(
        events: Vec<HookEventKind>,
        matcher: Matcher,
        handler: Arc<dyn HookHandler>,
    ) -> Self {
        Self {
            events,
            matcher,
            handler,
        }
    }

    /// Which event kinds this hook fires on.
    pub fn events(&self) -> &[HookEventKind] {
        &self.events
    }

    /// The matcher applied to events of these kinds.
    pub fn matcher(&self) -> &Matcher {
        &self.matcher
    }

    /// Does this registration match the given event?
    pub fn matches(&self, event: &HookEvent) -> bool {
        if !self.events.contains(&event.kind()) {
            return false;
        }
        // Events without a matcher value (UserPromptSubmit, Stop, …) always fire.
        match event.matcher_value() {
            None => true,
            Some(val) => self.matcher.matches_value(val),
        }
    }
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

#[cfg(test)]
mod tests;
