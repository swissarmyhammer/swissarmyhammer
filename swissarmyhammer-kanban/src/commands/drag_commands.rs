//! Drag session command implementations.
//!
//! These commands manage cross-window drag sessions in UIState.
//! The actual `drag-session-active` event is emitted by the Tauri layer
//! as a post-execution side effect (same pattern as BoardSwitch).

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError, DragSession};

/// Start a cross-window drag session.
///
/// Stores the drag session in UIState, replacing any existing session.
/// Returns a `DragStart` result that the Tauri dispatch layer uses to emit
/// the `drag-session-active` event to all windows.
///
/// Required args: `taskId`, `boardPath`
/// Optional args: `sourceWindowLabel` (defaults to "main"), `taskFields`, `copyMode`
pub struct DragStartCmd;

#[async_trait]
impl Command for DragStartCmd {
    /// Always available — drag can be started at any time.
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    /// Execute the drag.start command.
    ///
    /// Reads session parameters from `ctx.args`, cancels any existing session,
    /// stores the new session, and returns a `DragStart` result payload.
    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        // Frontend sends camelCase arg names
        let task_id = ctx
            .args
            .get("taskId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("taskId".into()))?
            .to_string();

        let source_board_path = ctx
            .args
            .get("boardPath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("boardPath".into()))?
            .to_string();

        let source_window_label = ctx
            .args
            .get("sourceWindowLabel")
            .and_then(|v| v.as_str())
            .unwrap_or("main")
            .to_string();

        let task_fields = ctx.args.get("taskFields").cloned().unwrap_or(Value::Null);

        let copy_mode = ctx
            .args
            .get("copyMode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let session_id = ulid::Ulid::new().to_string();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Cancel any existing session before starting a new one
        ui.cancel_drag();

        let session = DragSession {
            session_id: session_id.clone(),
            source_board_path: source_board_path.clone(),
            source_window_label: source_window_label.clone(),
            task_id: task_id.clone(),
            task_fields: task_fields.clone(),
            copy_mode,
            started_at_ms: now_ms,
        };

        ui.start_drag(session);

        // Return DragStart result — dispatch_command_internal emits drag-session-active
        Ok(json!({
            "DragStart": {
                "session_id": session_id,
                "source_board_path": source_board_path,
                "source_window_label": source_window_label,
                "task_id": task_id,
                "task_fields": task_fields,
                "copy_mode": copy_mode,
                "started_at_ms": now_ms,
            }
        }))
    }
}

/// Cancel the active drag session.
///
/// Takes the session from UIState and returns a `DragCancel` result.
/// The Tauri dispatch handler emits `drag-session-cancelled`.
/// Gracefully returns `null` if no session is active.
pub struct DragCancelCmd;

#[async_trait]
impl Command for DragCancelCmd {
    /// Always available — cancel can be called at any time (even with no active session).
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    /// Execute the drag.cancel command.
    ///
    /// Takes the active drag session from UIState.  If a session was active,
    /// returns a `DragCancel` result payload so the Tauri layer can emit
    /// `drag-session-cancelled`.  Returns `null` when no session is active.
    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        match ui.take_drag() {
            Some(session) => Ok(json!({
                "DragCancel": {
                    "session_id": session.session_id,
                }
            })),
            None => Ok(Value::Null),
        }
    }
}
