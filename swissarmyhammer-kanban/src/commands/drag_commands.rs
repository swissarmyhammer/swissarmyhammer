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
        let params = resolve_drag_complete_params(ctx)?;
        if params.session.source_board_path == params.target_board_path {
            complete_same_board(ctx, params).await
        } else {
            Ok(build_cross_board_payload(params))
        }
    }
}

/// Resolved parameters for a drag-complete invocation.
///
/// Bundles the active `DragSession` with the drop-target args extracted from
/// `CommandContext`, including the `effective_copy_mode` which combines the
/// session's initial copy flag with the frontend's drop-time flag (either
/// can trigger a copy).
struct DragCompleteParams {
    session: DragSession,
    target_board_path: String,
    target_column: String,
    drop_index: Option<u64>,
    before_id: Option<String>,
    after_id: Option<String>,
    effective_copy_mode: bool,
}

/// Validate the command context and extract drag-complete parameters.
///
/// Returns an error if `UIState` is missing, there is no active drag
/// session, the target board path cannot be resolved from the scope chain
/// or args, or the required `targetColumn` arg is absent.
fn resolve_drag_complete_params(
    ctx: &CommandContext,
) -> swissarmyhammer_commands::Result<DragCompleteParams> {
    let session = take_active_drag_session(ctx)?;
    let target_board_path = resolve_target_board_path(ctx)?;
    let target_column = ctx
        .args
        .get("targetColumn")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CommandError::MissingArg("targetColumn".into()))?
        .to_string();
    let (drop_index, before_id, after_id, copy_mode) = read_drop_target_args(ctx);

    // Combine frontend copy_mode with session copy_mode — either can request a copy
    let effective_copy_mode = copy_mode || session.copy_mode;

    Ok(DragCompleteParams {
        session,
        target_board_path,
        target_column,
        drop_index,
        before_id,
        after_id,
        effective_copy_mode,
    })
}

/// Take the currently-active drag session from `UIState`.
///
/// Errors when `UIState` is not attached to the command context (e.g. a
/// non-Tauri caller) or when no session is currently pending.
fn take_active_drag_session(ctx: &CommandContext) -> swissarmyhammer_commands::Result<DragSession> {
    let ui = ctx
        .ui_state
        .as_ref()
        .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;
    ui.take_drag()
        .ok_or_else(|| CommandError::ExecutionFailed("No active drag session".into()))
}

/// Resolve the target board path from the scope chain's `store:{path}`
/// moniker, falling back to the explicit `targetBoardPath` arg for
/// backwards compatibility.
fn resolve_target_board_path(ctx: &CommandContext) -> swissarmyhammer_commands::Result<String> {
    ctx.resolve_store_path()
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
        })
}

/// Read the optional drop-target args: `dropIndex`, `beforeId`, `afterId`,
/// and `copyMode`. Missing values produce `None`/`false`.
fn read_drop_target_args(
    ctx: &CommandContext,
) -> (Option<u64>, Option<String>, Option<String>, bool) {
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
    (drop_index, before_id, after_id, copy_mode)
}

/// Execute the same-board drag path: perform the `task.move` directly via
/// the `KanbanContext` extension and return the `DragComplete` payload.
///
/// Mirrors the logic in `dispatch_command_internal`'s same-board path.
async fn complete_same_board(
    ctx: &CommandContext,
    params: DragCompleteParams,
) -> swissarmyhammer_commands::Result<Value> {
    let kanban = ctx.require_extension::<crate::context::KanbanContext>()?;

    let mut op = crate::task::MoveTask::to_column(
        params.session.task_id.clone(),
        params.target_column.clone(),
    );

    // Ordinal resolution: before_id/after_id > drop_index > append at end
    if params.before_id.is_some() || params.after_id.is_some() {
        let ordinal = resolve_ordinal_from_neighbors(
            &kanban,
            &params.session.task_id,
            &params.target_column,
            params.before_id.as_deref(),
            params.after_id.as_deref(),
        )
        .await?;
        op = op.with_ordinal(ordinal.as_str());
    } else if let Some(idx) = params.drop_index {
        let ordinal = resolve_ordinal_from_drop_index(
            &kanban,
            &params.session.task_id,
            &params.target_column,
            idx as usize,
        )
        .await?;
        op = op.with_ordinal(ordinal.as_str());
    }

    let move_result = super::run_op(&op, &kanban).await?;

    Ok(json!({
        "DragComplete": {
            "session_id": params.session.session_id,
            "same_board": true,
            "task_id": params.session.task_id,
            "target_column": params.target_column,
            "move_result": move_result,
        }
    }))
}

/// Compute the ordinal for an insert anchored on a neighbouring task id.
///
/// Loads the target column's tasks (excluding the moving task), sorts them
/// by existing ordinal, then picks a fresh ordinal using
/// `compute_ordinal_for_neighbors`. `before_id` wins over `after_id` when
/// both are set; unknown reference ids fall through to "append at end".
async fn resolve_ordinal_from_neighbors(
    kanban: &crate::context::KanbanContext,
    moving_task_id: &str,
    target_column: &str,
    before_id: Option<&str>,
    after_id: Option<&str>,
) -> swissarmyhammer_commands::Result<crate::types::Ordinal> {
    let ectx = kanban
        .entity_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

    let all_tasks = ectx
        .list("task")
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

    let col_tasks = sort_column_tasks(all_tasks, target_column, moving_task_id);

    let ordinal = if let Some(ref_id) = before_id {
        ordinal_before(&col_tasks, ref_id)
    } else if let Some(ref_id) = after_id {
        ordinal_after(&col_tasks, ref_id)
    } else {
        crate::task_helpers::compute_ordinal_for_neighbors(None, None)
    };
    Ok(ordinal)
}

/// Filter `all_tasks` to tasks in `target_column` (excluding
/// `moving_task_id`) and sort them ascending by `position_ordinal`.
fn sort_column_tasks(
    all_tasks: Vec<swissarmyhammer_entity::Entity>,
    target_column: &str,
    moving_task_id: &str,
) -> Vec<swissarmyhammer_entity::Entity> {
    use crate::types::Ordinal;
    let mut col_tasks: Vec<_> = all_tasks
        .into_iter()
        .filter(|t| {
            t.get_str("position_column") == Some(target_column) && t.id.as_str() != moving_task_id
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
    col_tasks
}

/// Compute an ordinal slotting the moving task immediately before
/// `ref_id`. When `ref_id` is unknown, appends at the end of the column.
fn ordinal_before(
    col_tasks: &[swissarmyhammer_entity::Entity],
    ref_id: &str,
) -> crate::types::Ordinal {
    use crate::types::Ordinal;
    let ref_idx = col_tasks.iter().position(|t| t.id.as_str() == ref_id);
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
            crate::task_helpers::compute_ordinal_for_neighbors(Some(&pred_ord), Some(&ref_ord))
        }
        None => append_ordinal(col_tasks),
    }
}

/// Compute an ordinal slotting the moving task immediately after
/// `ref_id`. When `ref_id` is unknown, appends at the end of the column.
fn ordinal_after(
    col_tasks: &[swissarmyhammer_entity::Entity],
    ref_id: &str,
) -> crate::types::Ordinal {
    use crate::types::Ordinal;
    let ref_idx = col_tasks.iter().position(|t| t.id.as_str() == ref_id);
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
            crate::task_helpers::compute_ordinal_for_neighbors(Some(&ref_ord), Some(&succ_ord))
        }
        None => append_ordinal(col_tasks),
    }
}

/// Compute an ordinal that appends to the end of `col_tasks`.
fn append_ordinal(col_tasks: &[swissarmyhammer_entity::Entity]) -> crate::types::Ordinal {
    use crate::types::Ordinal;
    crate::task_helpers::compute_ordinal_for_neighbors(
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
    )
}

/// Compute the ordinal for an insert at `drop_index` within the target
/// column, using `compute_ordinal_for_drop`.
async fn resolve_ordinal_from_drop_index(
    kanban: &crate::context::KanbanContext,
    moving_task_id: &str,
    target_column: &str,
    drop_index: usize,
) -> swissarmyhammer_commands::Result<crate::types::Ordinal> {
    use crate::types::Ordinal;
    let all_tasks = kanban
        .list_entities_generic("task")
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let mut column_tasks: Vec<_> = all_tasks
        .into_iter()
        .filter(|t| {
            t.get_str("position_column") == Some(target_column) && t.id.as_str() != moving_task_id
        })
        .collect();
    column_tasks.sort_by(|a, b| {
        let oa = a
            .get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR);
        let ob = b
            .get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR);
        oa.cmp(ob)
    });
    Ok(crate::task_helpers::compute_ordinal_for_drop(
        &column_tasks,
        drop_index,
    ))
}

/// Build the cross-board `DragComplete` payload.
///
/// Cross-board transfers are executed by the Tauri dispatch handler (which
/// has both board handles). This helper just packages the parameters the
/// handler needs.
fn build_cross_board_payload(params: DragCompleteParams) -> Value {
    json!({
        "DragComplete": {
            "session_id": params.session.session_id,
            "same_board": false,
            "cross_board": true,
            "source_board_path": params.session.source_board_path,
            "target_board_path": params.target_board_path,
            "task_id": params.session.task_id,
            "target_column": params.target_column,
            "drop_index": params.drop_index,
            "before_id": params.before_id,
            "after_id": params.after_id,
            "copy_mode": params.effective_copy_mode,
        }
    })
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
    use swissarmyhammer_operations::Execute;

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
        assert!(!dc["copy_mode"].as_bool().unwrap());
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
        assert!(
            dc["copy_mode"].as_bool().unwrap(),
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
        assert!(
            dc["copy_mode"].as_bool().unwrap(),
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
    // DragCompleteCmd — same-board path (needs KanbanContext)
    // =========================================================================

    #[tokio::test]
    async fn drag_complete_same_board_moves_task() {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = crate::context::KanbanContext::new(kanban_dir.clone());

        // Init board
        crate::board::InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add a task
        let result = crate::task::AddTask::new("Draggable")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = result["id"].as_str().unwrap().to_string();

        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let board_path = kanban_dir.display().to_string();
        let session = DragSession {
            session_id: "same-board-s1".into(),
            source_board_path: board_path.clone(),
            source_window_label: "main".into(),
            task_id: task_id.clone(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 0,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!(board_path));
        args.insert("targetColumn".into(), json!("doing"));
        let mut cmd_ctx = CommandContext::new("drag.complete", vec![], None, args);
        cmd_ctx.ui_state = Some(ui.clone());
        cmd_ctx.set_extension(Arc::new(ctx));

        let result = cmd.execute(&cmd_ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        assert!(dc["same_board"].as_bool().unwrap());
        assert_eq!(dc["target_column"].as_str().unwrap(), "doing");
    }

    #[tokio::test]
    async fn drag_complete_same_board_with_drop_index() {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = crate::context::KanbanContext::new(kanban_dir.clone());

        crate::board::InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add two tasks in doing
        let mut op1 = crate::task::AddTask::new("D1");
        op1.column = Some("doing".into());
        op1.execute(&ctx).await.into_result().unwrap();

        let mut op2 = crate::task::AddTask::new("D2");
        op2.column = Some("doing".into());
        op2.execute(&ctx).await.into_result().unwrap();

        // Add task to drag
        let result = crate::task::AddTask::new("Dragger")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = result["id"].as_str().unwrap().to_string();

        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let board_path = kanban_dir.display().to_string();
        let session = DragSession {
            session_id: "drop-idx-s1".into(),
            source_board_path: board_path.clone(),
            source_window_label: "main".into(),
            task_id: task_id.clone(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 0,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!(board_path));
        args.insert("targetColumn".into(), json!("doing"));
        args.insert("dropIndex".into(), json!(0));
        let mut cmd_ctx = CommandContext::new("drag.complete", vec![], None, args);
        cmd_ctx.ui_state = Some(ui.clone());
        cmd_ctx.set_extension(Arc::new(ctx));

        let result = cmd.execute(&cmd_ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        assert!(dc["same_board"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn drag_complete_same_board_with_before_id() {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = crate::context::KanbanContext::new(kanban_dir.clone());

        crate::board::InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add a reference task in doing
        let mut ref_op = crate::task::AddTask::new("Reference");
        ref_op.column = Some("doing".into());
        let ref_result = ref_op.execute(&ctx).await.into_result().unwrap();
        let ref_id = ref_result["id"].as_str().unwrap().to_string();

        // Task to drag
        let result = crate::task::AddTask::new("Before mover")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = result["id"].as_str().unwrap().to_string();

        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let board_path = kanban_dir.display().to_string();
        let session = DragSession {
            session_id: "before-s1".into(),
            source_board_path: board_path.clone(),
            source_window_label: "main".into(),
            task_id: task_id.clone(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 0,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!(board_path));
        args.insert("targetColumn".into(), json!("doing"));
        args.insert("beforeId".into(), json!(ref_id));
        let mut cmd_ctx = CommandContext::new("drag.complete", vec![], None, args);
        cmd_ctx.ui_state = Some(ui.clone());
        cmd_ctx.set_extension(Arc::new(ctx));

        let result = cmd.execute(&cmd_ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        assert!(dc["same_board"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn drag_complete_same_board_with_after_id() {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = crate::context::KanbanContext::new(kanban_dir.clone());

        crate::board::InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add a reference task in doing
        let mut ref_op = crate::task::AddTask::new("Anchor");
        ref_op.column = Some("doing".into());
        let ref_result = ref_op.execute(&ctx).await.into_result().unwrap();
        let ref_id = ref_result["id"].as_str().unwrap().to_string();

        // Task to drag
        let result = crate::task::AddTask::new("After mover")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = result["id"].as_str().unwrap().to_string();

        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let board_path = kanban_dir.display().to_string();
        let session = DragSession {
            session_id: "after-s1".into(),
            source_board_path: board_path.clone(),
            source_window_label: "main".into(),
            task_id: task_id.clone(),
            task_fields: Value::Null,
            copy_mode: false,
            started_at_ms: 0,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!(board_path));
        args.insert("targetColumn".into(), json!("doing"));
        args.insert("afterId".into(), json!(ref_id));
        let mut cmd_ctx = CommandContext::new("drag.complete", vec![], None, args);
        cmd_ctx.ui_state = Some(ui.clone());
        cmd_ctx.set_extension(Arc::new(ctx));

        let result = cmd.execute(&cmd_ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        assert!(dc["same_board"].as_bool().unwrap());
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
