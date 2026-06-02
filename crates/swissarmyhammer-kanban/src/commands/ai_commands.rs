//! AI panel command implementations.
//!
//! These back the `ai.*` window-layer commands declared in
//! `builtin/commands/ai.yaml`: `ai.toggle`, `ai.focus`, `ai.newChat`,
//! `ai.model`, and `ai.cancel`.
//!
//! # Why the backend impls are no-ops
//!
//! The AI panel's open-state, conversation, and ACP session all live entirely
//! in the React tree (per board, in `localStorage` / `useConversation`). There
//! is no backend store for any of it — see the `AiPanelContainer` task. So the
//! frontend resolves a local `execute` handler for every `ai.*` command before
//! any dispatch reaches the backend, exactly like `ui.entity.startRename`.
//!
//! These impls exist only so the YAML <-> Rust completeness guard
//! (`register_commands` ↔ `builtin/commands/*.yaml`) is satisfied and the
//! palette / keybinding pipeline has a registered command to surface. Their
//! `execute` returns `Value::Null` — the frontend never lets the call through.
//!
//! # `ai.cancel` is the one real `available()` gate
//!
//! `ai.cancel` stops an in-flight generation, so it is meaningful only while
//! the conversation is streaming. The webview reports the streaming status
//! into a transient `UIState` flag (`set_ai_streaming`, the `can_undo`
//! precedent); `AiCancelCmd::available()` reads it so `commands_for_scope`
//! keeps the palette entry hidden when the conversation is idle. The other
//! four commands are always available.

use crate::commands_core::{Command, CommandContext};
use async_trait::async_trait;
use serde_json::Value;

/// Show or hide the AI panel.
///
/// Always available. Backend execution is a no-op — the frontend's local
/// `execute` handler flips the panel's `localStorage`-backed open-state.
pub struct AiToggleCmd;

#[async_trait]
impl Command for AiToggleCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        Ok(Value::Null)
    }
}

/// Move keyboard focus into the AI panel.
///
/// Always available. Backend execution is a no-op — the frontend's local
/// `execute` handler focuses the panel's prompt input.
pub struct AiFocusCmd;

#[async_trait]
impl Command for AiFocusCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        Ok(Value::Null)
    }
}

/// Start a fresh stateless AI chat, clearing the current conversation.
///
/// Always available. Backend execution is a no-op — the frontend's local
/// `execute` handler calls `useConversation`'s `newConversation`, dropping the
/// ACP session so the next prompt opens a brand-new `newSession`.
pub struct AiNewChatCmd;

#[async_trait]
impl Command for AiNewChatCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        Ok(Value::Null)
    }
}

/// Change the AI model (`:ai model <name>`).
///
/// Always available. Backend execution is a no-op — the frontend's local
/// `execute` handler reads the `model` arg and applies it via the panel's
/// per-board model-selection handler.
pub struct AiModelCmd;

#[async_trait]
impl Command for AiModelCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        Ok(Value::Null)
    }
}

/// Stop the in-flight AI generation.
///
/// Available **only** while the conversation is streaming — a generation can
/// only be cancelled mid-stream. The gate reads the transient
/// `UIState::ai_streaming` flag the webview keeps in sync with the ACP turn
/// status. Backend execution is a no-op — the frontend's local `execute`
/// handler calls `cancel` on the ACP client.
pub struct AiCancelCmd;

#[async_trait]
impl Command for AiCancelCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        // No UIState means we cannot observe the conversation — fail closed
        // (unavailable) so a non-functional "Stop" entry never shows.
        ctx.ui_state.as_ref().is_some_and(|ui| ui.ai_streaming())
    }

    async fn execute(&self, _ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_ui_state::UIState;

    /// Build a bare `CommandContext` with an empty scope chain and no UIState.
    fn ctx_bare() -> CommandContext {
        CommandContext::new("test", vec![], None, HashMap::new())
    }

    /// Build a `CommandContext` carrying a UIState whose `ai_streaming` flag
    /// is set to `streaming`.
    fn ctx_with_streaming(streaming: bool) -> CommandContext {
        let ui = Arc::new(UIState::new());
        ui.set_ai_streaming(streaming);
        CommandContext::new("test", vec![], None, HashMap::new()).with_ui_state(ui)
    }

    #[test]
    fn toggle_focus_new_chat_model_are_always_available() {
        let ctx = ctx_bare();
        assert!(AiToggleCmd.available(&ctx), "ai.toggle is always available");
        assert!(AiFocusCmd.available(&ctx), "ai.focus is always available");
        assert!(
            AiNewChatCmd.available(&ctx),
            "ai.newChat is always available"
        );
        assert!(AiModelCmd.available(&ctx), "ai.model is always available");
    }

    #[test]
    fn cancel_unavailable_when_idle() {
        // No UIState — fail closed.
        assert!(
            !AiCancelCmd.available(&ctx_bare()),
            "ai.cancel must be unavailable without UIState"
        );
        // UIState present but not streaming.
        assert!(
            !AiCancelCmd.available(&ctx_with_streaming(false)),
            "ai.cancel must be unavailable when the conversation is idle"
        );
    }

    #[test]
    fn cancel_available_while_streaming() {
        assert!(
            AiCancelCmd.available(&ctx_with_streaming(true)),
            "ai.cancel must be available while the conversation is streaming"
        );
    }

    #[tokio::test]
    async fn every_ai_command_executes_as_a_noop() {
        // Each backend `ai.*` impl is a deliberate no-op — it returns
        // `Value::Null` because the webview intercepts the command with a
        // local `execute` handler before any dispatch reaches the backend.
        let ctx = ctx_with_streaming(true);
        assert!(AiToggleCmd.execute(&ctx).await.unwrap().is_null());
        assert!(AiFocusCmd.execute(&ctx).await.unwrap().is_null());
        assert!(AiNewChatCmd.execute(&ctx).await.unwrap().is_null());
        assert!(AiModelCmd.execute(&ctx).await.unwrap().is_null());
        assert!(AiCancelCmd.execute(&ctx).await.unwrap().is_null());
    }
}
