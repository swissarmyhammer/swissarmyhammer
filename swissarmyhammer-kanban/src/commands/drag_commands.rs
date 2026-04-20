//! Drag session command implementations.
//!
//! These commands manage cross-window drag sessions in UIState.
//! The actual `drag-session-active` event is emitted by the Tauri layer
//! as a post-execution side effect (same pattern as BoardSwitch).

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError, DragSession, DragSource};

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

        let params = resolve_drag_start_params(ctx)?;

        // Cancel any existing session before starting a new one
        ui.cancel_drag();

        let session = DragSession {
            session_id: params.session_id.clone(),
            from: DragSource::FocusChain {
                entity_type: "task".to_string(),
                entity_id: params.task_id.clone(),
                fields: params.task_fields.clone(),
                source_board_path: params.source_board_path.clone(),
                source_window_label: params.source_window_label.clone(),
            },
            copy_mode: params.copy_mode,
            started_at_ms: params.started_at_ms,
        };

        ui.start_drag(session);

        // Return DragStart result — dispatch_command_internal emits drag-session-active.
        //
        // The wire payload preserves the legacy flat shape (`task_id`,
        // `source_board_path`, …) for the frontend's `drag-session-active`
        // event listener, even though the in-memory session now uses the
        // generalized `DragSource::FocusChain` enum. Future cards that add
        // non-task drag sources will extend the wire payload accordingly.
        Ok(json!({
            "DragStart": {
                "session_id": params.session_id,
                "source_board_path": params.source_board_path,
                "source_window_label": params.source_window_label,
                "task_id": params.task_id,
                "task_fields": params.task_fields,
                "copy_mode": params.copy_mode,
                "started_at_ms": params.started_at_ms,
            }
        }))
    }
}

/// Parameters for a drag-start invocation.
struct DragStartParams {
    session_id: String,
    task_id: String,
    source_board_path: String,
    source_window_label: String,
    task_fields: Value,
    copy_mode: bool,
    started_at_ms: u64,
}

/// Validate the command context and extract drag-start parameters.
fn resolve_drag_start_params(
    ctx: &CommandContext,
) -> swissarmyhammer_commands::Result<DragStartParams> {
    let task_id = ctx
        .args
        .get("taskId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CommandError::MissingArg("taskId".into()))?
        .to_string();

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

    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    Ok(DragStartParams {
        session_id,
        task_id,
        source_board_path,
        source_window_label,
        task_fields,
        copy_mode,
        started_at_ms,
    })
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
        // Same-board test compares the source board path captured on the
        // session against the resolved target board path. For non-focus-chain
        // sources (e.g. external files), there is no source board, so the
        // drag is treated as cross-board (or, in the future, dispatched via
        // the PasteMatrix as an "external paste").
        let same_board = params
            .session
            .source_board_path()
            .map(|src| src == params.target_board_path)
            .unwrap_or(false);
        if same_board {
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
/// `before_id` / `after_id` placement is delegated to `MoveTask::execute`
/// (the canonical ordinal computation path). Only `drop_index` is resolved
/// locally because `MoveTask` does not have a `drop_index` field.
///
/// This path is task-on-board specific: it preserves the dragged task's
/// identity (same id, same dependencies). The dispatcher only routes here
/// when the source is a `DragSource::FocusChain` whose `entity_type` is
/// `"task"` and the source/target board paths match — otherwise the drop
/// is cross-board (or, in the future, an external-source drag dispatched
/// via the `PasteMatrix`).
async fn complete_same_board(
    ctx: &CommandContext,
    params: DragCompleteParams,
) -> swissarmyhammer_commands::Result<Value> {
    let kanban = ctx.require_extension::<crate::context::KanbanContext>()?;

    let task_id = params.session.entity_id().ok_or_else(|| {
        CommandError::ExecutionFailed(
            "drag.complete same-board path requires a focus-chain source".into(),
        )
    })?;
    // Defensive guard — DragStartCmd only constructs `entity_type = "task"`
    // sources today, but the FocusChain enum allows other types. Surface
    // a clear error rather than silently moving a non-task entity through
    // the task.move op.
    if let Some(t) = params.session.entity_type() {
        if t != "task" {
            return Err(CommandError::ExecutionFailed(format!(
                "drag.complete same-board path only handles tasks, got '{t}'"
            )));
        }
    }

    let mut op =
        crate::task::MoveTask::to_column(task_id.to_string(), params.target_column.clone());

    // Ordinal resolution: before_id/after_id > drop_index > append at end
    // Delegate before/after placement to MoveTask::execute (single canonical path).
    if let Some(ref before_id) = params.before_id {
        op = op.with_before(before_id.as_str());
    } else if let Some(ref after_id) = params.after_id {
        op = op.with_after(after_id.as_str());
    } else if let Some(idx) = params.drop_index {
        let ordinal =
            resolve_ordinal_from_drop_index(&kanban, task_id, &params.target_column, idx as usize)
                .await?;
        op = op.with_ordinal(ordinal.as_str());
    }

    let move_result = super::run_op(&op, &kanban).await?;

    Ok(json!({
        "DragComplete": {
            "session_id": params.session.session_id,
            "same_board": true,
            "task_id": task_id,
            "target_column": params.target_column,
            "move_result": move_result,
        }
    }))
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
///
/// External-source drags (`DragSource::File`) are not yet wired into
/// cross-board transfer; this helper writes empty strings for the source
/// path and id in that case so the Tauri handler's `transfer_task` produces
/// a clear validation error rather than panicking on a `None` extraction.
fn build_cross_board_payload(params: DragCompleteParams) -> Value {
    let source_board_path = params.session.source_board_path().unwrap_or("").to_string();
    let task_id = params.session.entity_id().unwrap_or("").to_string();
    json!({
        "DragComplete": {
            "session_id": params.session.session_id,
            "same_board": false,
            "cross_board": true,
            "source_board_path": source_board_path,
            "target_board_path": params.target_board_path,
            "task_id": task_id,
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

    /// Build a focus-chain `DragSession` for tests.
    ///
    /// All callers in this module construct task-source sessions; this
    /// helper wraps the boilerplate of the new `DragSource::FocusChain`
    /// variant so the assertions stay focused on the behavior under test.
    fn task_drag_session(
        session_id: &str,
        source_board_path: &str,
        task_id: &str,
        task_fields: Value,
        copy_mode: bool,
        started_at_ms: u64,
    ) -> DragSession {
        DragSession {
            session_id: session_id.into(),
            from: DragSource::FocusChain {
                entity_type: "task".into(),
                entity_id: task_id.into(),
                fields: task_fields,
                source_board_path: source_board_path.into(),
                source_window_label: "main".into(),
            },
            copy_mode,
            started_at_ms,
        }
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
        assert_eq!(session.source_window_label(), Some("secondary"));
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
        let session = task_drag_session("s1", "/boards/a", "task-1", Value::Null, false, 0);
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
        let session = task_drag_session("s1", "/boards/a", "task-1", Value::Null, false, 0);
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
        let session = task_drag_session("s1", "/boards/a", "task-1", Value::Null, false, 100);
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
        let session = task_drag_session("s2", "/boards/a", "task-2", Value::Null, true, 200);
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
        let session = task_drag_session("s3", "/boards/a", "task-3", Value::Null, false, 300);
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
        let session = task_drag_session("s4", "/boards/a", "task-4", Value::Null, false, 400);
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
        let session = task_drag_session("s5", "/boards/a", "task-5", Value::Null, false, 500);
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
        let session = task_drag_session(
            "cancel-s1",
            "/boards/a",
            "task-x",
            serde_json::Value::Null,
            false,
            999,
        );
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
        assert_eq!(new_session.entity_id(), Some("task-2"));
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
        let session = task_drag_session(
            "same-board-s1",
            &board_path,
            &task_id,
            Value::Null,
            false,
            0,
        );
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
        let session =
            task_drag_session("drop-idx-s1", &board_path, &task_id, Value::Null, false, 0);
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
        let session = task_drag_session("before-s1", &board_path, &task_id, Value::Null, false, 0);
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
        let session = task_drag_session("after-s1", &board_path, &task_id, Value::Null, false, 0);
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
        assert_eq!(session.entity_id(), Some("task-1"));
        assert_eq!(session.source_board_path(), Some("/boards/a"));
    }

    // =========================================================================
    // Generalized DragSource shape — verifies the from/to refactor preserves
    // the legacy behavior while exposing the entity-type field that future
    // non-task drag sources will populate.
    // =========================================================================

    /// `DragStartCmd` always constructs a `DragSource::FocusChain` whose
    /// `entity_type` is `"task"` today. This test pins that contract so a
    /// future "DragSource::File" addition cannot accidentally relabel
    /// existing task drags.
    #[tokio::test]
    async fn drag_start_constructs_focus_chain_task_source() {
        let cmd = DragStartCmd;
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("boardPath".into(), json!("/boards/a"));
        args.insert("taskId".into(), json!("task-7"));
        args.insert("taskFields".into(), json!({"title": "Hello"}));
        let ctx = ctx_with_args_and_ui(args, ui.clone());

        cmd.execute(&ctx).await.unwrap();
        let session = ui.drag_session().unwrap();
        // Match on the `from` enum directly to lock the variant shape.
        match session.from {
            DragSource::FocusChain {
                entity_type,
                entity_id,
                fields,
                source_board_path,
                source_window_label,
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(entity_id, "task-7");
                assert_eq!(fields["title"].as_str().unwrap(), "Hello");
                assert_eq!(source_board_path, "/boards/a");
                assert_eq!(source_window_label, "main");
            }
            DragSource::File { .. } => panic!("expected FocusChain, got File"),
        }
    }

    /// The accessor methods on `DragSession` round-trip through the new
    /// enum-shaped `from` field. Callers (DragCompleteCmd, the cross-board
    /// payload builder) rely on these accessors instead of touching the
    /// enum directly.
    #[tokio::test]
    async fn drag_session_accessors_round_trip_focus_chain_fields() {
        let session =
            task_drag_session("s-acc", "/boards/x", "task-acc", json!({"a": 1}), true, 42);
        assert_eq!(session.entity_type(), Some("task"));
        assert_eq!(session.entity_id(), Some("task-acc"));
        assert_eq!(session.source_board_path(), Some("/boards/x"));
        assert_eq!(session.source_window_label(), Some("main"));
        let fields = session.fields().unwrap();
        assert_eq!(fields["a"].as_i64().unwrap(), 1);
        assert!(session.copy_mode);
        assert_eq!(session.started_at_ms, 42);
    }

    /// External-source drags (`DragSource::File`) are not yet emitted by
    /// `DragStartCmd`, but the enum shape must still flow through
    /// `DragCompleteCmd`'s same-board guard cleanly. Without a focus-chain
    /// source, the same-board path is a no-go and the dispatcher falls
    /// through to the cross-board payload (which carries empty source
    /// fields so the Tauri handler can surface a clear validation error).
    #[tokio::test]
    async fn drag_complete_external_source_falls_through_to_cross_board() {
        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let session = DragSession {
            session_id: "ext-s1".into(),
            from: DragSource::File {
                path: "/tmp/dragged.png".into(),
            },
            copy_mode: false,
            started_at_ms: 0,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!("/boards/b"));
        args.insert("targetColumn".into(), json!("done"));
        let ctx = ctx_with_args_and_ui(args, ui);

        let result = cmd.execute(&ctx).await.unwrap();
        let dc = result.get("DragComplete").unwrap();
        // No focus-chain source means same-board comparison fails — the
        // result is a cross-board payload with empty source fields.
        assert!(!dc["same_board"].as_bool().unwrap());
        assert!(dc["cross_board"].as_bool().unwrap());
        assert_eq!(dc["source_board_path"].as_str().unwrap(), "");
        assert_eq!(dc["task_id"].as_str().unwrap(), "");
        assert_eq!(dc["target_board_path"].as_str().unwrap(), "/boards/b");
    }

    /// Future-proofing: a non-`task` `DragSource::FocusChain` must NOT
    /// silently flow through `complete_same_board`'s `task.move` op.
    /// `DragStartCmd` only constructs `entity_type = "task"` today, but
    /// the enum allows other types — guard against that quietly succeeding.
    #[tokio::test]
    async fn drag_complete_same_board_rejects_non_task_focus_chain() {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let kctx = crate::context::KanbanContext::new(kanban_dir.clone());
        crate::board::InitBoard::new("Test")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();

        let cmd = DragCompleteCmd;
        let ui = Arc::new(UIState::new());
        let board_path = kanban_dir.display().to_string();
        let session = DragSession {
            session_id: "non-task-s1".into(),
            from: DragSource::FocusChain {
                entity_type: "tag".into(),
                entity_id: "tag-1".into(),
                fields: Value::Null,
                source_board_path: board_path.clone(),
                source_window_label: "main".into(),
            },
            copy_mode: false,
            started_at_ms: 0,
        };
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!(board_path));
        args.insert("targetColumn".into(), json!("doing"));
        let mut cmd_ctx = CommandContext::new("drag.complete", vec![], None, args);
        cmd_ctx.ui_state = Some(ui);
        cmd_ctx.set_extension(Arc::new(kctx));

        let result = cmd.execute(&cmd_ctx).await;
        assert!(
            result.is_err(),
            "non-task focus-chain source must error rather than running task.move"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("only handles tasks"),
            "expected 'only handles tasks' guard message, got: {err_msg}"
        );
    }

    /// Drag-complete with `beforeId = first_task` produces the same ordinal
    /// as `DoThisNextCmd` on an identical board, since both now delegate to
    /// `MoveTask::execute`'s `before_id` logic.
    #[tokio::test]
    async fn drag_before_first_matches_do_this_next() {
        use crate::board::InitBoard;
        use crate::task::AddTask;

        // Board A: DoThisNextCmd path
        let temp_a = tempfile::TempDir::new().unwrap();
        let kanban_dir_a = temp_a.path().join(".kanban");
        let kctx_a = crate::context::KanbanContext::new(kanban_dir_a.clone());
        InitBoard::new("Test")
            .execute(&kctx_a)
            .await
            .into_result()
            .unwrap();

        let mut anchor_a = AddTask::new("Anchor");
        anchor_a.column = Some("todo".into());
        anchor_a.execute(&kctx_a).await.into_result().unwrap();

        let mut target_a = AddTask::new("Target");
        target_a.column = Some("doing".into());
        let target_a_result = target_a.execute(&kctx_a).await.into_result().unwrap();
        let target_a_id = target_a_result["id"].as_str().unwrap().to_string();

        // Execute DoThisNextCmd via the Command trait
        let kanban_a = Arc::new(kctx_a);
        let dtn_ctx = {
            let mut ctx = CommandContext::new(
                "task.do-this-next",
                vec![format!("task:{target_a_id}")],
                None,
                HashMap::new(),
            );
            ctx.set_extension(Arc::clone(&kanban_a));
            ctx
        };
        let dtn_result = super::super::task_commands::DoThisNextCmd
            .execute(&dtn_ctx)
            .await
            .unwrap();
        let dtn_ordinal = dtn_result["position"]["ordinal"]
            .as_str()
            .unwrap()
            .to_string();

        // Board B: DragCompleteCmd with beforeId path
        let temp_b = tempfile::TempDir::new().unwrap();
        let kanban_dir_b = temp_b.path().join(".kanban");
        let kctx_b = crate::context::KanbanContext::new(kanban_dir_b.clone());
        InitBoard::new("Test")
            .execute(&kctx_b)
            .await
            .into_result()
            .unwrap();

        let mut anchor_b = AddTask::new("Anchor");
        anchor_b.column = Some("todo".into());
        anchor_b.execute(&kctx_b).await.into_result().unwrap();
        let anchor_b_id = {
            let tasks = kctx_b.list_entities_generic("task").await.unwrap();
            tasks
                .iter()
                .find(|t| t.get_str("position_column") == Some("todo"))
                .unwrap()
                .id
                .as_str()
                .to_string()
        };

        let mut target_b = AddTask::new("Target");
        target_b.column = Some("doing".into());
        let target_b_result = target_b.execute(&kctx_b).await.into_result().unwrap();
        let target_b_id = target_b_result["id"].as_str().unwrap().to_string();

        let board_path_b = kanban_dir_b.display().to_string();
        let ui = Arc::new(UIState::new());
        let session = task_drag_session(
            "parity-s1",
            &board_path_b,
            &target_b_id,
            Value::Null,
            false,
            0,
        );
        ui.start_drag(session);

        let mut args = HashMap::new();
        args.insert("targetBoardPath".into(), json!(board_path_b));
        args.insert("targetColumn".into(), json!("todo"));
        args.insert("beforeId".into(), json!(anchor_b_id));
        let mut cmd_ctx = CommandContext::new("drag.complete", vec![], None, args);
        cmd_ctx.ui_state = Some(ui.clone());
        cmd_ctx.set_extension(Arc::new(kctx_b));

        let drag_result = DragCompleteCmd.execute(&cmd_ctx).await.unwrap();
        let dc = drag_result.get("DragComplete").unwrap();
        let drag_ordinal = dc["move_result"]["position"]["ordinal"]
            .as_str()
            .unwrap()
            .to_string();

        // Both paths must produce the same ordinal
        assert_eq!(
            dtn_ordinal, drag_ordinal,
            "DoThisNextCmd ordinal {dtn_ordinal:?} must match drag-before-first ordinal {drag_ordinal:?}"
        );
    }
}
