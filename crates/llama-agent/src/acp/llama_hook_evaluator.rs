//! [`LlamaHookEvaluator`]: a llama-model-backed [`HookEvaluator`].
//!
//! `type: prompt` and `type: agent` hooks (parsed by `HookConfig`) need an
//! `Arc<dyn HookEvaluator>` at `build_registrations` time. This module provides
//! one backed by the llama model so those hooks evaluate live.
//!
//! # The decision contract
//!
//! The [`HookEvaluator`] contract is intentionally narrow: given a prompt and an
//! `is_agent` flag, return a JSON string of the shape
//! `{ "ok": bool, "reason"?: string }`. The hook *handlers*
//! (`PromptHandler` / `AgentHandler` in `agent-client-protocol-extras`) own the
//! timeout, the JSON parsing, and the mapping to a `HookDecision`; this
//! evaluator's only job is to run the model and hand back that JSON. Per the
//! handler fallback, anything this evaluator cannot turn into a clean decision
//! degrades to `{ "ok": true }` (Allow) — a hook evaluation must never crash a
//! turn.
//!
//! # The two paths
//!
//! `is_agent` selects how the evaluation runs. A `type: prompt` hook
//! (`is_agent=false`) takes a single short model call. A `type: agent` hook
//! (`is_agent=true`) takes a bounded multi-turn tool loop — the model may run
//! the session's tools across several rounds (capped by
//! [`HOOK_AGENT_MAX_TURNS`]) to *investigate* before committing to a verdict.
//! Both paths funnel their raw output through the same defensive normalization
//! and Allow-on-error degradation, so the never-crash-the-turn contract holds
//! either way.
//!
//! # The generation seam
//!
//! The genuine model work (feed a rendered prompt into llama.cpp, read tokens
//! back, dispatch tools) lives behind [`ModelManager::with_model`] and the
//! agent's tool loop, neither of which has a weight-free test double. To keep
//! the evaluator's *logic* — instruction wrapping, path selection, defensive
//! JSON extraction, error degradation — testable in milliseconds without a GPU,
//! both the single short call and the bounded tool loop are captured behind the
//! [`HookModel`] trait. [`AgentServer`] implements it for production (the prompt
//! path mirrors `AgentServer::title_via_model`; the agent path reuses the live
//! tool loop via `AgentServer::generate_agent_short`); tests implement it with a
//! scripted fake. This is the same seam philosophy as the crate's
//! `ScriptedModel` [`TextGenerator`](crate::generation::TextGenerator) double.

use std::sync::Arc;

use agent_client_protocol_extras::HookEvaluator;
use async_trait::async_trait;

use crate::agent::AgentServer;

/// Token budget for a single hook evaluation.
///
/// A verdict is a tiny JSON object; capping generation tightly keeps a runaway
/// model from turning a hook into a long-running call. Mirrors the small-call
/// budget used by `AgentServer::title_via_model`.
const HOOK_EVAL_MAX_TOKENS: usize = 256;

/// Maximum tool-call rounds an `is_agent=true` evaluation may take.
///
/// The agent path lets the model run tools before deciding, but a hook must
/// never become an unbounded agent run. This caps the number of tool rounds;
/// once spent, the loop forces a final verdict generation. The bound itself is
/// exercised by the unit tests that drive the production `run_bounded_tool_loop`
/// (`agent::tests::bounded_tool_loop`), so this stays crate-private.
pub(crate) const HOOK_AGENT_MAX_TURNS: usize = 8;

/// System instruction forcing the model to answer with the decision JSON.
///
/// Kept terse and explicit so even a small model reliably emits a parseable
/// object. The handler treats unparseable output as Allow, so the instruction
/// is an optimization, not a correctness requirement.
const HOOK_EVAL_INSTRUCTION: &str =
    "You are a hook evaluator. Decide whether the action described \
     below should be allowed. Reply with ONLY a JSON object and nothing else, in exactly this \
     form: {\"ok\": true} to allow, or {\"ok\": false, \"reason\": \"<short reason>\"} to block. \
     Do not include any prose, markdown, or code fences.";

/// The single short model call a hook evaluation needs.
///
/// This is the seam between [`LlamaHookEvaluator`]'s deterministic logic and the
/// GPU-bound generation step. Production is [`AgentServer`]; tests use a scripted
/// fake. Implementors run the model with `system_instruction` constraining the
/// output and `user_prompt` carrying the hook input, generate at most
/// `max_tokens`, and return the raw generated text.
#[async_trait]
pub trait HookModel: Send + Sync {
    /// Run one short, bounded generation and return the raw model output.
    ///
    /// # Parameters
    /// - `system_instruction`: constrains the model to emit decision JSON.
    /// - `user_prompt`: the hook input (already `$ARGUMENTS`-substituted by the handler).
    /// - `max_tokens`: hard cap on generated tokens.
    ///
    /// # Errors
    /// Returns `Err(message)` if the model is unavailable or generation fails.
    /// Callers degrade such errors to an Allow decision rather than propagating.
    async fn generate_eval(
        &self,
        system_instruction: &str,
        user_prompt: &str,
        max_tokens: usize,
    ) -> Result<String, String>;

    /// Run a bounded multi-turn agent loop and return the raw verdict text.
    ///
    /// This backs the `is_agent=true` path: the model may run the session's
    /// tools across up to `max_turns` rounds before committing to a verdict,
    /// letting an agent hook *investigate* rather than answer blind. Each
    /// generation is capped at `max_tokens`; once the turn budget is spent the
    /// implementation forces a final verdict generation so the call still
    /// terminates.
    ///
    /// # Parameters
    /// - `system_instruction`: constrains the model's final output to decision JSON.
    /// - `user_prompt`: the hook input (already `$ARGUMENTS`-substituted by the handler).
    /// - `max_turns`: hard cap on tool-call rounds.
    /// - `max_tokens`: per-turn hard cap on generated tokens.
    ///
    /// # Errors
    /// Returns `Err(message)` if the loop cannot run or a generation fails.
    /// Callers degrade such errors to an Allow decision rather than propagating.
    async fn generate_agent_eval(
        &self,
        system_instruction: &str,
        user_prompt: &str,
        max_turns: usize,
        max_tokens: usize,
    ) -> Result<String, String>;
}

/// A [`HookEvaluator`] that decides hooks by calling the llama model.
///
/// Construct one with [`LlamaHookEvaluator::new`] passing an `Arc<AgentServer>`
/// (production) — the returned `Arc<Self>` coerces to `Arc<dyn HookEvaluator>`
/// for `hookable_agent_from_config`. The type is generic over the [`HookModel`]
/// seam so tests can drive it with a scripted model.
pub struct LlamaHookEvaluator<M: HookModel> {
    model: Arc<M>,
}

impl LlamaHookEvaluator<AgentServer> {
    /// Build a production evaluator backed by an [`AgentServer`]'s model.
    ///
    /// Returns an `Arc` so it can be handed straight to
    /// `hookable_agent_from_config` as `Some(evaluator)`.
    pub fn new(agent: Arc<AgentServer>) -> Arc<Self> {
        Arc::new(Self { model: agent })
    }
}

impl<M: HookModel> LlamaHookEvaluator<M> {
    /// Build an evaluator over an arbitrary [`HookModel`] seam.
    ///
    /// Primarily for tests that supply a scripted model; production uses
    /// [`LlamaHookEvaluator::new`].
    pub fn with_model(model: Arc<M>) -> Arc<Self> {
        Arc::new(Self { model })
    }
}

#[async_trait]
impl<M: HookModel> HookEvaluator for LlamaHookEvaluator<M> {
    /// Evaluate a hook prompt, returning a `{ "ok": bool, "reason"?: string }`
    /// JSON string.
    ///
    /// `is_agent` selects the path: `false` runs a single bounded model call
    /// (the prompt path); `true` runs a bounded multi-turn tool loop (the agent
    /// path), letting the model investigate with the session's tools before
    /// committing to a verdict. Either way, a generation failure or output that
    /// contains no recoverable decision JSON degrades to `{ "ok": true }` so a
    /// hook can never crash the turn — matching the handler's own
    /// unparseable-output fallback.
    async fn evaluate(&self, prompt: &str, is_agent: bool) -> Result<String, String> {
        let raw = if is_agent {
            self.model
                .generate_agent_eval(
                    HOOK_EVAL_INSTRUCTION,
                    prompt,
                    HOOK_AGENT_MAX_TURNS,
                    HOOK_EVAL_MAX_TOKENS,
                )
                .await
        } else {
            self.model
                .generate_eval(HOOK_EVAL_INSTRUCTION, prompt, HOOK_EVAL_MAX_TOKENS)
                .await
        };

        match raw {
            Ok(text) => Ok(normalize_decision_json(&text)),
            Err(e) => {
                tracing::warn!(error = %e, "Hook model call failed; allowing");
                Ok(allow_json())
            }
        }
    }
}

/// The canonical "allow" response.
fn allow_json() -> String {
    r#"{"ok":true}"#.to_string()
}

/// Coerce raw model output into a clean decision JSON string.
///
/// Tries, in order: the whole trimmed output, then the first balanced
/// `{ ... }` object embedded in prose. A candidate counts only if it parses as
/// the expected `{ "ok": bool, "reason"?: string }` shape. Anything else
/// degrades to the allow response, mirroring the handler's treat-unparseable-as-Allow
/// behaviour.
fn normalize_decision_json(raw: &str) -> String {
    if let Some(json) = extract_decision(raw.trim()) {
        return json;
    }
    if let Some(candidate) = first_json_object(raw) {
        if let Some(json) = extract_decision(&candidate) {
            return json;
        }
    }
    allow_json()
}

/// Parse `candidate` as the decision shape and re-serialize it canonically.
///
/// Returns `None` when it is not a valid `{ "ok": bool, "reason"?: string }`
/// object, so callers can fall through to the next strategy or the allow
/// default.
fn extract_decision(candidate: &str) -> Option<String> {
    let parsed: agent_client_protocol_extras::PromptHookResponse =
        serde_json::from_str(candidate).ok()?;
    serde_json::to_string(&parsed).ok()
}

/// Extract the first balanced `{ ... }` object from `text`, if any.
///
/// Walks the text tracking brace depth so a complete top-level object is
/// returned even when it is surrounded by prose. String contents (including
/// escaped quotes) are skipped so braces inside a JSON string do not throw off
/// the depth count.
fn first_json_object(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let start = bytes.iter().position(|&b| b == b'{')?;

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, &b) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..=start + offset].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[async_trait]
impl HookModel for AgentServer {
    /// Run the hook-evaluation model call through the live llama model.
    ///
    /// Reuses [`AgentServer::generate_short`], the same `with_model` short-call
    /// path that backs session-title generation, with the hook system
    /// instruction in place of the title instruction.
    async fn generate_eval(
        &self,
        system_instruction: &str,
        user_prompt: &str,
        max_tokens: usize,
    ) -> Result<String, String> {
        let max_tokens = u32::try_from(max_tokens).unwrap_or(u32::MAX);
        self.generate_short(system_instruction, user_prompt, max_tokens)
            .await
            .map_err(|e| e.to_string())
    }

    /// Run the agent-hook evaluation through the live llama tool loop.
    ///
    /// Reuses [`AgentServer::generate_agent_short`], the bounded counterpart to
    /// the short-call path: it stands up an ephemeral session with the agent's
    /// tools, lets the model run them for up to `max_turns` rounds, then returns
    /// the model's final verdict text.
    async fn generate_agent_eval(
        &self,
        system_instruction: &str,
        user_prompt: &str,
        max_turns: usize,
        max_tokens: usize,
    ) -> Result<String, String> {
        let max_tokens = u32::try_from(max_tokens).unwrap_or(u32::MAX);
        self.generate_agent_short(system_instruction, user_prompt, max_turns, max_tokens)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_passes_through_clean_allow() {
        let out = normalize_decision_json(r#"{"ok": true}"#);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
    }

    #[test]
    fn normalize_passes_through_clean_block_with_reason() {
        let out = normalize_decision_json(r#"{"ok": false, "reason": "nope"}"#);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["reason"], "nope");
    }

    #[test]
    fn normalize_extracts_embedded_object() {
        let out = normalize_decision_json("verdict: {\"ok\": false, \"reason\": \"x\"} done");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["reason"], "x");
    }

    #[test]
    fn normalize_garbage_degrades_to_allow() {
        let out = normalize_decision_json("no json here at all");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
    }

    #[test]
    fn normalize_non_decision_object_degrades_to_allow() {
        // A well-formed JSON object that is not the decision shape must not be
        // accepted as a verdict.
        let out = normalize_decision_json(r#"{"status": "fine"}"#);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
    }

    #[test]
    fn first_json_object_skips_braces_in_strings() {
        let found = first_json_object(r#"pre {"reason": "has } brace"} post"#).unwrap();
        assert_eq!(found, r#"{"reason": "has } brace"}"#);
    }

    #[test]
    fn first_json_object_none_when_absent() {
        assert!(first_json_object("nothing here").is_none());
    }
}
