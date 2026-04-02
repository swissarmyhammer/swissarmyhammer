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

/// Complete an active drag session by dropping in a target column.
///
/// Reads the active drag session from UIState, determines whether the drop is
/// same-board or cross-board, and returns a `DragComplete` result payload.
///
/// **Same-board**: performs the task.move operation directly via `MoveTaskCmd`
/// logic using the `KanbanContext` extension.
///
/// **Cross-board**: returns a `DragComplete` result with `cross_board: true` and
/// all the transfer parameters. The Tauri `dispatch_command_internal` handler
/// calls `swissarmyhammer_kanban::cross_board::transfer_task()` with both board
/// handles, then emits the appropriate events.
///
/// Required args: `targetBoardPath`, `targetColumn`
/// Optional args: `dropIndex`, `beforeId`, `afterId`, `copyMode`
pub struct DragCompleteCmd;

#[async_trait]
impl Command for DragCompleteCmd {
    /// Always available — drag can be completed at any time.
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    /// Execute the drag.complete command.
    ///
    /// Takes the active drag session, checks if same-board or cross-board,
    /// performs the operation, and returns a `DragComplete` result payload.
    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let session = ui
            .take_drag()
            .ok_or_else(|| CommandError::ExecutionFailed("No active drag session".into()))?;

        let target_board_path = ctx
            .args
            .get("targetBoardPath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("targetBoardPath".into()))?
            .to_string();

        let target_column = ctx
            .args
            .get("targetColumn")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("targetColumn".into()))?
            .to_string();

        let drop_index = ctx.args.get("dropIndex").and_then(|v| v.as_u64());
        let before_id = ctx
            .args
            .get("beforeId")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let after_id = ctx
            .args
            .get("afterId")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let copy_mode = ctx
            .args
            .get("copyMode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Combine frontend copy_mode with session copy_mode — either can request a copy
        let effective_copy_mode = copy_mode || session.copy_mode;

        if session.source_board_path == target_board_path {
            // Same-board: perform task.move directly using KanbanContext extension.
            // This mirrors the logic in dispatch_command_internal (same-board path).
            let kanban = ctx.require_extension::<crate::context::KanbanContext>()?;

            let mut op =
                crate::task::MoveTask::to_column(session.task_id.clone(), target_column.clone());

            // Ordinal resolution: before_id/after_id > drop_index > append at end
            if before_id.is_some() || after_id.is_some() {
                use crate::types::Ordinal;

                let ectx = kanban
                    .entity_context()
                    .await
                    .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

                let all_tasks = ectx
                    .list("task")
                    .await
                    .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

                // Sort tasks in target column, excluding the moving task
                let mut col_tasks: Vec<_> = all_tasks
                    .into_iter()
                    .filter(|t| {
                        t.get_str("position_column") == Some(&target_column)
                            && t.id.as_str() != session.task_id.as_str()
                    })
                    .collect();
                col_tasks.sort_by(|a, b| {
                    let oa = a
                        .get_str("position_ordinal")
                        .unwrap_or(Ordinal::DEFAULT_STR);
                    let ob = b
                        .get_str("position_ordinal")
                        .unwrap_or(Ordinal::DEFAULT_STR);
                    oa.cmp(ob)
                });

                let ordinal = if let Some(ref ref_id) = before_id {
                    let ref_idx = col_tasks
                        .iter()
                        .position(|t| t.id.as_str() == ref_id.as_str());
                    match ref_idx {
                        Some(0) => {
                            let ref_ord = Ordinal::from_string(
                                col_tasks[0]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            crate::task_helpers::compute_ordinal_for_neighbors(None, Some(&ref_ord))
                        }
                        Some(idx) => {
                            let pred_ord = Ordinal::from_string(
                                col_tasks[idx - 1]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            let ref_ord = Ordinal::from_string(
                                col_tasks[idx]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            crate::task_helpers::compute_ordinal_for_neighbors(
                                Some(&pred_ord),
                                Some(&ref_ord),
                            )
                        }
                        None => crate::task_helpers::compute_ordinal_for_neighbors(
                            col_tasks
                                .last()
                                .map(|t| {
                                    Ordinal::from_string(
                                        t.get_str("position_ordinal")
                                            .unwrap_or(Ordinal::DEFAULT_STR),
                                    )
                                })
                                .as_ref(),
                            None,
                        ),
                    }
                } else if let Some(ref ref_id) = after_id {
                    let ref_idx = col_tasks
                        .iter()
                        .position(|t| t.id.as_str() == ref_id.as_str());
                    match ref_idx {
                        Some(idx) if idx == col_tasks.len() - 1 => {
                            let ref_ord = Ordinal::from_string(
                                col_tasks[idx]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            crate::task_helpers::compute_ordinal_for_neighbors(Some(&ref_ord), None)
                        }
                        Some(idx) => {
                            let ref_ord = Ordinal::from_string(
                                col_tasks[idx]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            let succ_ord = Ordinal::from_string(
                                col_tasks[idx + 1]
                                    .get_str("position_ordinal")
                                    .unwrap_or(Ordinal::DEFAULT_STR),
                            );
                            crate::task_helpers::compute_ordinal_for_neighbors(
                                Some(&ref_ord),
                                Some(&succ_ord),
                            )
                        }
                        None => crate::task_helpers::compute_ordinal_for_neighbors(
                            col_tasks
                                .last()
                                .map(|t| {
                                    Ordinal::from_string(
                                        t.get_str("position_ordinal")
                                            .unwrap_or(Ordinal::DEFAULT_STR),
                                    )
                                })
                                .as_ref(),
                            None,
                        ),
                    }
                } else {
                    crate::task_helpers::compute_ordinal_for_neighbors(None, None)
                };

                op = op.with_ordinal(ordinal.as_str());
            } else if let Some(idx) = drop_index {
                let all_tasks = kanban
                    .list_entities_generic("task")
                    .await
                    .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
                let mut column_tasks: Vec<_> = all_tasks
                    .into_iter()
                    .filter(|t| {
                        t.get_str("position_column") == Some(&target_column)
                            && t.id != session.task_id
                    })
                    .collect();
                column_tasks.sort_by(|a, b| {
                    let oa = a
                        .get_str("position_ordinal")
                        .unwrap_or(crate::types::Ordinal::DEFAULT_STR);
                    let ob = b
                        .get_str("position_ordinal")
                        .unwrap_or(crate::types::Ordinal::DEFAULT_STR);
                    oa.cmp(ob)
                });
                let ordinal =
                    crate::task_helpers::compute_ordinal_for_drop(&column_tasks, idx as usize);
                op = op.with_ordinal(ordinal.as_str());
            }

            let move_result = super::run_op(&op, &kanban).await?;

            Ok(json!({
                "DragComplete": {
                    "session_id": session.session_id,
                    "same_board": true,
                    "task_id": session.task_id,
                    "target_column": target_column,
                    "move_result": move_result,
                }
            }))
        } else {
            // Cross-board: return transfer parameters for Tauri layer to handle.
            // The Tauri dispatch handler will call cross_board::transfer_task()
            // with both board handles, then call flush_and_emit for both boards.
            Ok(json!({
                "DragComplete": {
                    "session_id": session.session_id,
                    "same_board": false,
                    "cross_board": true,
                    "source_board_path": session.source_board_path,
                    "target_board_path": target_board_path,
                    "task_id": session.task_id,
                    "target_column": target_column,
                    "drop_index": drop_index,
                    "before_id": before_id,
                    "after_id": after_id,
                    "copy_mode": effective_copy_mode,
                }
            }))
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::{CommandContext, UIState};

    /// Build a CommandContext with the given scope chain, target, and optional UIState.
    fn ctx_with(scope: &[&str], target: Option<&str>, ui: Option<Arc<UIState>>) -> CommandContext {
        let mut ctx = CommandContext::new(
            "test",
            scope.iter().map(|s| s.to_string()).collect(),
            target.map(|s| s.to_string()),
            HashMap::new(),
        );
        if let Some(ui) = ui {
            ctx.ui_state = Some(ui);
        }
        ctx
    }

    fn ctx_with_args_and_ui(args: HashMap<String, Value>, ui: Arc<UIState>) -> CommandContext {
        let mut ctx = CommandContext::new("test", vec![], None, args);
        ctx.ui_state = Some(ui);
        ctx
    }

    // =========================================================================
    // Availability tests
    // =========================================================================

    #[test]
    fn drag_start_always_available() {
        let cmd = DragStartCmd;
        let ctx = ctx_with(&[], None, None);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn drag_start_available_with_any_scope() {
        let cmd = DragStartCmd;
        let ctx = ctx_with(&["task:01ABC", "column:todo"], None, None);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn drag_complete_always_available() {
        let cmd = DragCompleteCmd;
        let ctx = ctx_with(&[], None, None);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn drag_cancel_always_available() {
        let cmd = DragCancelCmd;
        let ctx = ctx_with(&[], None, None);
        assert!(cmd.available(&ctx));
    }

    #[test]
    fn drag_cancel_available_with_scope() {
        let cmd = DragCancelCmd;
        let ctx = ctx_with(&["task:01ABC"], None, None);
        assert!(cmd.available(&ctx));
    }

    // =========================================================================
    // DragStartCmd execute error cases
    // =========================================================================

    #[tokio::test]
    async fn drag_start_fails_without_ui_state() {
        let cmd = DragStartCmd;
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        let ctx = CommandContext::new("drag.start", vec![], None, args);
        // No ui_state set
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without UIState");
    }

    #[tokio::test]
    async fn drag_start_fails_without_board_path() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("taskId".into(), json!("task-1"));
        // boardPath intentionally omitted
        let ctx = ctx_with_args_and_ui(args, ui);
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without boardPath");
    }

    #[tokio::test]
    async fn drag_start_custom_source_window_label() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        args.insert("sourceWindowLabel".into(), json!("secondary"));
        let ctx = ctx_with_args_and_ui(args, ui.clone());

        let result = cmd.execute(&ctx).await.unwrap();
        let ds = result.get("DragStart").unwrap();
        assert_eq!(ds["source_window_label"].as_str().unwrap(), "secondary");

        let session = ui.drag_session().unwrap();
        assert_eq!(session.source_window_label, "secondary");
    }

    #[tokio::test]
    async fn drag_start_preserves_task_fields() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        args.insert(
            "taskFields".into(),
            json!({"title": "Hello", "status": "open"}),
        );
        let ctx = ctx_with_args_and_ui(args, ui.clone());

        let result = cmd.execute(&ctx).await.unwrap();
        let ds = result.get("DragStart").unwrap();
        assert_eq!(ds["task_fields"]["title"].as_str().unwrap(), "Hello");
        assert_eq!(ds["task_fields"]["status"].as_str().unwrap(), "open");
    }

    #[tokio::test]
    async fn drag_start_task_fields_defaults_to_null() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        // taskFields not provided
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let ds = result.get("DragStart").unwrap();
        assert!(ds["task_fields"].is_null());
    }

    #[tokio::test]
    async fn drag_start_copy_mode_in_result() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        args.insert("copyMode".into(), json!(true));
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let ds = result.get("DragStart").unwrap();
        assert!(ds["copy_mode"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn drag_start_result_has_session_id() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let ds = result.get("DragStart").unwrap();
        let sid = ds["session_id"].as_str().unwrap();
        assert!(!sid.is_empty(), "session_id should be non-empty");
    }

    #[tokio::test]
    async fn drag_start_result_has_started_at_ms() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let ds = result.get("DragStart").unwrap();
        assert!(ds["started_at_ms"].as_u64().unwrap() > 0);
    }

    // =========================================================================
    // DragCompleteCmd execute error cases
    // =========================================================================

    #[tokio::test]
    async fn drag_complete_fails_without_ui_state() {
        let cmd = DragCompleteCmd;
        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/b"));
        args.insert("targetColumn".into(), json!("done"));
        let ctx = CommandContext::new("drag.complete", vec![], None, args);
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without UIState");
    }

    #[tokio::test]
    async fn drag_complete_fails_without_active_session() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/b"));
        args.insert("targetColumn".into(), json!("done"));
        let ctx = ctx_with_args_and_ui(args, ui);
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without active drag session");
    }

    #[tokio::test]
    async fn drag_complete_fails_without_target_board_path() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        // Start a session first
        let session = DragSession {
            session_id: "s1".into(),
            source_board_path: "/boards/a".into(),
            source_window_label: "main".into(),
            task_id: "task-1".into(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 0,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        // targetBoardPath intentionally omitted
        args.insert("targetColumn".into(), json!("done"));
        let ctx = ctx_with_args_and_ui(args, ui);
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without targetBoardPath");
    }

    #[tokio::test]
    async fn drag_complete_fails_without_target_column() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let session = DragSession {
            session_id: "s1".into(),
            source_board_path: "/boards/a".into(),
            source_window_label: "main".into(),
            task_id: "task-1".into(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 0,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/b"));
        // targetColumn intentionally omitted
        let ctx = ctx_with_args_and_ui(args, ui);
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without targetColumn");
    }

    #[tokio::test]
    async fn drag_complete_cross_board_returns_transfer_params() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let session = DragSession {
            session_id: "s1".into(),
            source_board_path: "/boards/a".into(),
            source_window_label: "main".into(),
            task_id: "task-1".into(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 100,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/b"));
        args.insert("targetColumn".into(), json!("done"));
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        assert!(!dc["same_board"].as_bool().unwrap());
        assert!(dc["cross_board"].as_bool().unwrap());
        assert_eq!(dc["source_board_path"].as_str().unwrap(), "/boards/a");
        assert_eq!(dc["target_board_path"].as_str().unwrap(), "/boards/b");
        assert_eq!(dc["task_id"].as_str().unwrap(), "task-1");
        assert_eq!(dc["target_column"].as_str().unwrap(), "done");
        assert_eq!(dc["copy_mode"].as_bool().unwrap(), false);
    }

    #[tokio::test]
    async fn drag_complete_cross_board_with_copy_mode_from_session() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let session = DragSession {
            session_id: "s2".into(),
            source_board_path: "/boards/a".into(),
            source_window_label: "main".into(),
            task_id: "task-2".into(),
            task_fields: Value::Null,
            copy_mode: true,
            started_at_ms: 200,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/c"));
        args.insert("targetColumn".into(), json!("todo"));
        // copyMode not set in args, but session has copy_mode=true
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        assert_eq!(
            dc["copy_mode"].as_bool().unwrap(),
            true,
            "effective_copy_mode should be true from session"
        );
    }

    #[tokio::test]
    async fn drag_complete_cross_board_with_copy_mode_from_args() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let session = DragSession {
            session_id: "s3".into(),
            source_board_path: "/boards/a".into(),
            source_window_label: "main".into(),
            task_id: "task-3".into(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 300,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/d"));
        args.insert("targetColumn".into(), json!("todo"));
        args.insert("copyMode".into(), json!(true));
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        assert_eq!(
            dc["copy_mode"].as_bool().unwrap(),
            true,
            "effective_copy_mode should be true from args"
        );
    }

    #[tokio::test]
    async fn drag_complete_cross_board_with_before_after_ids() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let session = DragSession {
            session_id: "s4".into(),
            source_board_path: "/boards/a".into(),
            source_window_label: "main".into(),
            task_id: "task-4".into(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 400,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/e"));
        args.insert("targetColumn".into(), json!("doing"));
        args.insert("beforeId".into(), json!("ref-task-1"));
        args.insert("afterId".into(), json!("ref-task-2"));
        args.insert("dropIndex".into(), json!(3));
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        assert_eq!(dc["before_id"].as_str().unwrap(), "ref-task-1");
        assert_eq!(dc["after_id"].as_str().unwrap(), "ref-task-2");
        assert_eq!(dc["drop_index"].as_u64().unwrap(), 3);
    }

    #[tokio::test]
    async fn drag_complete_consumes_session() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let session = DragSession {
            session_id: "s5".into(),
            source_board_path: "/boards/a".into(),
            source_window_label: "main".into(),
            task_id: "task-5".into(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 500,
        };
        ui.start_drag(session);
        assert!(ui.drag_session().is_some());

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/f"));
        args.insert("targetColumn".into(), json!("done"));
        let ctx = ctx_with_args_and_ui(args, ui.clone());

        cmd.execute(&ctx).await.unwrap();
        // Session should be consumed by take_drag
        assert!(ui.drag_session().is_none());
    }

    // =========================================================================
    // DragCancelCmd execute error cases
    // =========================================================================

    #[tokio::test]
    async fn drag_cancel_fails_without_ui_state() {
        let cmd = DragCancelCmd;
        let ctx = CommandContext::new("drag.cancel", vec![], None, HashMap::new());
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without UIState");
    }

    #[tokio::test]
    async fn drag_cancel_returns_drag_cancel_result_when_session_active() {
        let cmd = DragCancelCmd;
        let ui = Arc::new(UIState::new());
        let session = DragSession {
            session_id: "cancel-s1".into(),
            source_board_path: "/boards/a".into(),
            source_window_label: "main".into(),
            task_id: "task-x".into(),
            task_fields: serde_json::Value::Null,
            copy_mode: false,
            started_at_ms: 999,
        };
        ui.start_drag(session);

        let ctx = ctx_with_args_and_ui(HashMap::new(), ui.clone());
        let result = cmd.execute(&ctx).await.unwrap();

        let dc = result
            .get("DragCancel")
            .expect("should have DragCancel key");
        assert_eq!(dc["session_id"].as_str().unwrap(), "cancel-s1");

        // Session should be consumed
        assert!(ui.drag_session().is_none(), "session should be taken");
    }

    #[tokio::test]
    async fn drag_cancel_returns_null_when_no_session_active() {
        let cmd = DragCancelCmd;
        let ui = Arc::new(UIState::new());
        // No session started

        let ctx = ctx_with_args_and_ui(HashMap::new(), ui);
        let result = cmd.execute(&ctx).await.unwrap();

        assert!(
            result.is_null(),
            "should return null when no session is active, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn drag_cancel_cancels_existing_session_before_new_drag_start() {
        // Verify that starting a new drag cancels any existing session (from DragStartCmd).
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());

        // Start first drag
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        let ctx1 = ctx_with_args_and_ui(args.clone(), ui.clone());
        cmd.execute(&ctx1).await.unwrap();

        let first_session = ui.drag_session().unwrap();
        let first_id = first_session.session_id.clone();

        // Start second drag — DragStartCmd calls cancel_drag() then start_drag()
        let mut args2 = HashMap::new();
        args2.insert("boardPath".into(), json!("/boards/b"));
        args2.insert("taskId".into(), json!("task-2"));
        let ctx2 = ctx_with_args_and_ui(args2, ui.clone());
        cmd.execute(&ctx2).await.unwrap();

        let new_session = ui.drag_session().unwrap();
        assert_ne!(
            first_id, new_session.session_id,
            "new session should replace the old one"
        );
        assert_eq!(new_session.task_id, "task-2");
    }

    // =========================================================================
    // DragStartCmd — missing arg tests
    // =========================================================================

    #[tokio::test]
    async fn drag_start_fails_without_task_id() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        // taskId intentionally omitted
        let ctx = ctx_with_args_and_ui(args, ui);
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without taskId");
    }

    #[tokio::test]
    async fn drag_start_default_source_window_label_is_main() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        // sourceWindowLabel not provided — should default to "main"
        let ctx = ctx_with_args_and_ui(args, ui.clone());

        let result = cmd.execute(&ctx).await.unwrap();
        let ds = result.get("DragStart").unwrap();
        assert_eq!(ds["source_window_label"].as_str().unwrap(), "main");
    }

    #[tokio::test]
    async fn drag_start_stores_session_in_ui_state() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-1"));
        let ctx = ctx_with_args_and_ui(args, ui.clone());

        assert!(ui.drag_session().is_none(), "no session before execute");
        cmd.execute(&ctx).await.unwrap();
        let session = ui
            .drag_session()
            .expect("session should be set after execute");
        assert_eq!(session.task_id, "task-1");
        assert_eq!(session.source_board_path, "/boards/a");
    }
}
