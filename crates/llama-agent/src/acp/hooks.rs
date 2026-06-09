//! Per-session Claude-Code hook wiring for the ACP server lifecycle.
//!
//! [`HookableAgent`] and its lifecycle helpers
//! (`track_session_start` / `run_user_prompt_submit` / `run_stop`) live in
//! `agent-client-protocol-extras`, but nothing instantiates them for the llama
//! agent. This module is the bridge: it loads the `.claude/settings.json` hook
//! config for a session's cwd, builds the runtime registrations once per
//! session, and exposes the lifecycle seams the [`AcpServer`](super::server::AcpServer)
//! fires at `new_session`/`load_session` (SessionStart), at `prompt` entry
//! (UserPromptSubmit), and at `prompt` return (Stop).
//!
//! # Why per session, keyed by cwd
//!
//! Hooks are cwd-scoped: the user-level `~/.claude/settings.json` is global, but
//! the project-level `<cwd>/.claude/settings.json` depends on the session's
//! working directory, and each ACP session carries its own cwd. So the
//! [`HookConfig`](agent_client_protocol_extras::HookConfig) is loaded *per
//! session* from that session's cwd (via
//! [`load_hook_config`](agent_client_protocol_extras::load_hook_config)) and the
//! registrations are built once and cached, keyed by ACP session id.
//!
//! # The inner is `()`
//!
//! The `AcpServer` drives the ACP connection through `connect_with` (SDK 0.11),
//! not the `ConnectTo` middleware stack, so [`HookableAgent`] is never inserted
//! as transport middleware here. Only its standalone helper methods are called.
//! Those helpers do not touch the wrapped inner component, so the inner is the
//! unit type `()` — the wrapper exists purely to own the registrations and fire
//! hooks at the seams.
//!
//! # Cost when there are no hooks
//!
//! A session whose cwd has no `.claude` settings (and no user-level settings)
//! yields an empty [`HookConfig`], hence a [`HookableAgent`] with zero
//! registrations. Firing an event against it matches nothing and returns
//! immediately, so a hook-free session pays only one cheap settings-chain read
//! at session start and nothing per prompt beyond an empty fan-out.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_client_protocol::schema::SessionId as AcpSessionId;
use agent_client_protocol_extras::{
    hookable_agent_from_config_with_context, load_hook_config, raw_transcript_path,
    HookCommandContext, HookEvaluator, HookableAgent, SessionSource,
};
use tokio::sync::RwLock;

use super::permissions::PermissionPolicy;
use crate::acp::llama_hook_evaluator::LlamaHookEvaluator;
use crate::agent::AgentServer;

/// A [`HookableAgent`] with a unit inner — see the module docs for why the
/// inner is `()`.
pub(crate) type SessionHookAgent = HookableAgent<()>;

/// Owns and caches the per-session [`HookableAgent`]s for an
/// [`AcpServer`](super::server::AcpServer).
///
/// One [`SessionHooks`] is held by the server; it lazily builds a
/// [`HookableAgent`] the first time a session starts (keyed by ACP session id)
/// and hands it back at the prompt seams. The hook config is read from the
/// session's cwd exactly once, at session start.
pub(crate) struct SessionHooks {
    /// The llama [`AgentServer`] backing `type: prompt` / `type: agent` hook
    /// evaluation. Wrapped into a [`LlamaHookEvaluator`] per session so prompt
    /// and agent hooks can call the model.
    agent_server: Arc<AgentServer>,

    /// The permission-mode string surfaced to command hooks via
    /// [`HookCommandContext`](agent_client_protocol_extras::HookCommandContext),
    /// derived once from the server's permission policy.
    permission_mode: String,

    /// ACP session id → its built [`HookableAgent`]. Populated on the first
    /// session-start for that id and reused for every later prompt.
    by_session: RwLock<HashMap<AcpSessionId, Arc<SessionHookAgent>>>,
}

impl SessionHooks {
    /// Build a [`SessionHooks`] for a server with the given agent backend and
    /// permission policy.
    pub(crate) fn new(agent_server: Arc<AgentServer>, policy: &PermissionPolicy) -> Self {
        Self {
            agent_server,
            permission_mode: permission_mode_string(policy).to_string(),
            by_session: RwLock::new(HashMap::new()),
        }
    }

    /// Record a session's cwd, build its [`HookableAgent`] from the cwd's
    /// `.claude` settings chain, and fire its `SessionStart` hooks.
    ///
    /// Idempotent per session id: the registrations are built and the
    /// `SessionStart` hooks fired only on the first call for a given id;
    /// subsequent calls are a no-op so a resume after a load does not re-fire.
    ///
    /// `source` distinguishes a fresh `new_session`
    /// ([`SessionSource::Startup`]) from a `load_session` / `resume_session`
    /// ([`SessionSource::Resume`]).
    pub(crate) async fn track_session_start(
        &self,
        session_id: &AcpSessionId,
        source: SessionSource,
        cwd: PathBuf,
    ) {
        {
            // Fast path: already built for this session — nothing to do.
            if self.by_session.read().await.contains_key(session_id) {
                return;
            }
        }

        // Resolve the session's raw transcript path (`<acp-session-dir>/raw.jsonl`)
        // so command hooks receive `transcript_path` and can read the transcript,
        // matching Claude Code's hook contract. The server wires the matching
        // RawMessageManager to this same path at session start; a failure to
        // resolve it (an unusable session dir) degrades to no transcript path
        // rather than failing the session.
        let transcript_path = raw_transcript_path(session_id.0.as_ref()).ok();
        let agent = self.build_agent(&cwd, transcript_path.as_deref());
        let agent = Arc::new(agent);

        // Insert before firing so a concurrent prompt observes the registrations.
        // Re-check under the write lock in case a racing call already inserted.
        {
            let mut guard = self.by_session.write().await;
            if guard.contains_key(session_id) {
                return;
            }
            guard.insert(session_id.clone(), Arc::clone(&agent));
        }

        agent
            .track_session_start(session_id.0.to_string(), source, cwd)
            .await;
    }

    /// Return the built [`HookableAgent`] for a session, if one exists.
    ///
    /// Present once [`track_session_start`](Self::track_session_start) has run
    /// for the session. `None` for an unknown session means the prompt seams
    /// behave as if there were no hooks.
    pub(crate) async fn for_session(
        &self,
        session_id: &AcpSessionId,
    ) -> Option<Arc<SessionHookAgent>> {
        self.by_session.read().await.get(session_id).cloned()
    }

    /// Build a [`HookableAgent`] for a cwd from its `.claude` settings chain.
    ///
    /// Loads the merged [`HookConfig`](agent_client_protocol_extras::HookConfig)
    /// for `cwd`, wires a per-session [`LlamaHookEvaluator`] so prompt/agent
    /// hooks can call the model, and decorates the wrapper with the command
    /// context (permission mode, and `transcript_path` when one is available so
    /// command hooks can read the session transcript). A config that fails to
    /// build registrations — e.g. an empty hook list or an invalid matcher
    /// regex — degrades to a hook-free agent rather than failing the session,
    /// matching the loader's own never-fail-the-agent contract.
    ///
    /// # Parameters
    ///
    /// * `cwd` - The session's working directory; its `.claude` settings chain
    ///   is the hook source.
    /// * `transcript_path` - The session's raw transcript path
    ///   (`<acp-session-dir>/raw.jsonl`), or `None` when it could not be
    ///   resolved; passed to command hooks as `transcript_path`.
    fn build_agent(&self, cwd: &Path, transcript_path: Option<&Path>) -> SessionHookAgent {
        let config = load_hook_config(cwd);
        let evaluator: Arc<dyn HookEvaluator> =
            LlamaHookEvaluator::new(Arc::clone(&self.agent_server));

        // The context is captured into the handlers at build time, so command
        // hooks see `transcript_path`/`permission_mode` in their JSON stdin.
        // Builder methods on the wrapper only retag it and would not reach the
        // already-built handlers, so it must be threaded in here.
        let command_context = HookCommandContext {
            transcript_path: transcript_path
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            permission_mode: self.permission_mode.clone(),
        };

        match hookable_agent_from_config_with_context((), &config, Some(evaluator), command_context)
        {
            Ok(agent) => agent,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to build hook registrations for session cwd {:?}; running without hooks",
                    cwd
                );
                HookableAgent::new(())
            }
        }
    }
}

/// Map an ACP [`PermissionPolicy`] to a Claude-Code permission-mode string.
///
/// Claude Code's hook input carries a `permission_mode` field whose canonical
/// values are `default`, `acceptEdits`, `bypassPermissions`, and `plan`. The
/// llama agent's policy model is coarser, so this maps:
/// - [`PermissionPolicy::AlwaysAsk`] → `"default"` (every operation prompts), and
/// - [`PermissionPolicy::AutoApproveReads`] / [`PermissionPolicy::RuleBased`] →
///   `"acceptEdits"` (some operations are auto-approved).
///
/// The value is informational for command hooks; it does not gate firing.
fn permission_mode_string(policy: &PermissionPolicy) -> &'static str {
    match policy {
        PermissionPolicy::AlwaysAsk => "default",
        PermissionPolicy::AutoApproveReads | PermissionPolicy::RuleBased(_) => "acceptEdits",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_mode_maps_always_ask_to_default() {
        assert_eq!(
            permission_mode_string(&PermissionPolicy::AlwaysAsk),
            "default"
        );
    }

    #[test]
    fn permission_mode_maps_auto_approve_to_accept_edits() {
        assert_eq!(
            permission_mode_string(&PermissionPolicy::AutoApproveReads),
            "acceptEdits"
        );
    }

    #[test]
    fn permission_mode_maps_rule_based_to_accept_edits() {
        assert_eq!(
            permission_mode_string(&PermissionPolicy::RuleBased(Vec::new())),
            "acceptEdits"
        );
    }
}
