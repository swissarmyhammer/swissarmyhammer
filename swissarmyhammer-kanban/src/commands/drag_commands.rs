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
/// The source board path is derived from the scope chain's `store:{path}`
/// moniker — no explicit `boardPath` arg is needed.
///
/// Required args: `taskId`
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

        // Derive board path from scope chain's store:{path} moniker.
        // Falls back to explicit boardPath arg for backwards compatibility.
        let source_board_path = ctx
            .resolve_store_path()
            .map(|s| s.to_string())
            .or_else(|| {
                ctx.args
                    .get("boardPath")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| {
                CommandError::ExecutionFailed(
                    "No store path in scope chain and no boardPath arg".into(),
                )
            })?;

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
/// The target board path is derived from the scope chain's `store:{path}`
/// moniker — no explicit `targetBoardPath` arg is needed.
///
/// **Same-board**: performs the task.move operation directly via `MoveTaskCmd`
/// logic using the `KanbanContext` extension.
///
/// **Cross-board**: returns a `DragComplete` result with `cross_board: true` and
/// all the transfer parameters. The Tauri `dispatch_command_internal` handler
/// calls `swissarmyhammer_kanban::cross_board::transfer_task()` with both board
/// handles, then emits the appropriate events.
///
/// Required args: `targetColumn`
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

        // Derive target board path from scope chain's store:{path} moniker.
        // Falls back to explicit targetBoardPath arg for backwards compatibility.
        let target_board_path = ctx
            .resolve_store_path()
            .map(|s| s.to_string())
            .or_else(|| {
                ctx.args
                    .get("targetBoardPath")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| {
                CommandError::ExecutionFailed(
                    "No store path in scope chain and no targetBoardPath arg".into(),
                )
            })?;

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
