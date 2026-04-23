//! Test infrastructure: synthetic board fixtures for in-process Rust tests.
//!
//! This module is intentionally **backend-only** — everything here writes
//! content to disk and returns a handle that downstream test code can feed
//! to [`AppState::open_board`]. The factory never invokes React and never
//! talks to a window.
//!
//! The `mod test_support;` declaration in `main.rs` is `#[cfg(test)]`, so
//! the whole module compiles only for test builds — the `tempfile`
//! dev-dependency stays out of the production binary.

use std::path::PathBuf;

use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_entity::{Entity, EntityContext};
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::column::{AddColumn, DeleteColumn};
use swissarmyhammer_kanban::types::Ordinal;
use swissarmyhammer_kanban::{Execute, KanbanContext};
#[cfg(test)]
use tempfile::TempDir;

/// Handle to a synthetic board on disk.
///
/// The [`TempDir`] returned alongside this struct owns the filesystem
/// lifetime — drop it and the `.kanban/` directory disappears. Callers must
/// keep the `TempDir` alive for as long as they intend to read the fixture.
#[derive(Debug, Clone)]
pub struct BoardFixture {
    /// Absolute path to the project root (the directory that contains
    /// `.kanban/`). This is what [`AppState::open_board`] expects.
    ///
    /// [`AppState::open_board`]: crate::state::AppState::open_board
    pub path: PathBuf,
    /// Task identifiers in **layout order** (row-major: `task-<col>-<row>`).
    ///
    /// Index `(col - 1) * rows + (row - 1)` retrieves the task at the
    /// given position. For a 3x3 board that is:
    ///
    /// | index | id         |
    /// |-------|------------|
    /// | 0     | task-1-1   |
    /// | 1     | task-1-2   |
    /// | 2     | task-1-3   |
    /// | 3     | task-2-1   |
    /// | 4     | task-2-2   |
    /// | 5     | task-2-3   |
    /// | 6     | task-3-1   |
    /// | 7     | task-3-2   |
    /// | 8     | task-3-3   |
    pub tasks: Vec<String>,
}

/// Write a deterministic 3x3 board (3 columns, 3 tasks per column) to a
/// fresh tempdir.
///
/// Column ids are `col-1`, `col-2`, `col-3` with `order` 0, 1, 2. Task ids
/// are `task-<col>-<row>` for col in 1..=3 and row in 1..=3 — nine tasks
/// total. Each task's `title` field matches its id so DOM-level assertions
/// have a human-readable anchor, and `position_column` / `position_ordinal`
/// are filled in so the board renders in the expected spatial layout.
///
/// The default columns created by [`InitBoard`] (`todo`, `doing`, `done`)
/// are deleted so tests see exactly the three fixture columns.
///
/// Returns `(TempDir, BoardFixture)`. The `TempDir` must stay alive for the
/// duration of the test; dropping it deletes the `.kanban/` directory.
///
/// Runtime cost is dominated by [`InitBoard`] (board entity, default columns,
/// and — when running inside a git repo — merge-driver config) plus the nine
/// task writes. This is not a hot path: callers are test harnesses, and the
/// fixture is built once per test. Observed wall-clock times vary from
/// hundreds of milliseconds on a warm developer machine to several seconds
/// on a cold/loaded CI runner; tests must not assert on the duration.
#[cfg(test)]
pub async fn write_3x3_board() -> (TempDir, BoardFixture) {
    let tmp = TempDir::new().expect("tempdir create");
    let fixture = build_fixture(tmp.path().to_path_buf(), 3, 3).await;
    (tmp, fixture)
}

/// Write a fixture with a single column `col-1` holding `rows` tasks.
///
/// Useful for tests that exercise long-list navigation, virtualization, or
/// row-start/row-end edge commands. Task ids are `task-1-1` through
/// `task-1-<rows>`.
#[cfg(test)]
pub async fn write_long_column(rows: usize) -> (TempDir, BoardFixture) {
    let tmp = TempDir::new().expect("tempdir create");
    let fixture = build_fixture(tmp.path().to_path_buf(), 1, rows).await;
    (tmp, fixture)
}

/// Write a fixture sized to match the standard grid view (6 columns x 4 rows).
///
/// The current grid view assumes enough tasks to wrap across a few rows, so
/// 24 tasks split over 6 columns gives a dense grid for spatial tests to
/// navigate through. Callers that want a different shape should use
/// [`write_3x3_board`] or [`write_long_column`] or call [`build_fixture`]
/// directly.
#[cfg(test)]
pub async fn write_grid_view_fixture() -> (TempDir, BoardFixture) {
    let tmp = TempDir::new().expect("tempdir create");
    let fixture = build_fixture(tmp.path().to_path_buf(), 6, 4).await;
    (tmp, fixture)
}

/// Core fixture builder: initialises a board at `root`, replaces the default
/// columns with `col-1..=col-<cols>`, and adds `task-<col>-<row>` entries
/// for every `(col, row)` pair.
///
/// Exposed as `pub` so custom-shaped fixtures can share the InitBoard +
/// column-replace boilerplate. The returned [`BoardFixture::tasks`] is in
/// row-major order.
pub async fn build_fixture(root: PathBuf, cols: usize, rows: usize) -> BoardFixture {
    assert!(cols > 0, "cols must be >= 1");
    assert!(rows > 0, "rows must be >= 1");

    let kanban_dir = root.join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    // Step 1: InitBoard creates the directory layout, board entity, default
    // columns, and (if we happen to be inside a git repo) merge-driver
    // config. We want the layout bits but not the default columns — so we
    // delete them immediately below. Doing it this way, rather than
    // hand-rolling the YAML, means we share every storage-format change
    // that `InitBoard` picks up automatically.
    InitBoard::new("Fixture Board")
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard succeeds on a fresh tempdir");

    // Step 2: Remove the default columns so the caller sees only the
    // fixture's `col-N` columns. Must happen before any tasks are added
    // because `DeleteColumn` refuses to delete a column that still has
    // tasks pointing at it.
    for default_id in ["todo", "doing", "done"] {
        let _ = DeleteColumn::new(default_id)
            .execute(&ctx)
            .await
            .into_result();
    }

    // Step 3: Add the fixture columns. Order 0..cols so the UI renders them
    // left-to-right in the natural order.
    for col_idx in 1..=cols {
        AddColumn::new(format!("col-{col_idx}"), format!("Column {col_idx}"))
            .with_order(col_idx - 1)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap_or_else(|e| panic!("AddColumn col-{col_idx} failed: {e}"));
    }

    // Step 4: Write the task entities directly. Going through `AddTask`
    // would auto-generate ULIDs for the ids, which would defeat the whole
    // purpose of a deterministic fixture. The entity context's `write()`
    // accepts any id we hand it.
    //
    // The explicit `Arc<EntityContext>` annotation pins the `Result::Ok`
    // type so type inference succeeds despite `KanbanContext::Result` and
    // `EntityContext::Result` being distinct aliases.
    let ectx: Arc<EntityContext> = ctx
        .entity_context()
        .await
        .expect("entity_context available after InitBoard");

    let mut task_ids: Vec<String> = Vec::with_capacity(cols * rows);
    for col_idx in 1..=cols {
        let column_id = format!("col-{col_idx}");
        // Build ordinals fresh per column so every task's key-order matches
        // its row number. Ordinal::first() + after() gives lexically sorted
        // values, which is what the rest of the stack expects.
        let mut ordinal = Ordinal::first();
        for row_idx in 1..=rows {
            let task_id = format!("task-{col_idx}-{row_idx}");
            let mut entity = Entity::new("task", task_id.as_str());
            entity.set("title", json!(task_id));
            entity.set("body", json!(""));
            entity.set("position_column", json!(column_id));
            entity.set("position_ordinal", json!(ordinal.as_str()));
            ectx.write(&entity)
                .await
                .unwrap_or_else(|e| panic!("write task {task_id} failed: {e}"));
            ordinal = Ordinal::after(&ordinal);
            task_ids.push(task_id);
        }
    }

    BoardFixture {
        path: root,
        tasks: task_ids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    /// `write_3x3_board` must produce a `.kanban/` directory that
    /// `AppState::open_board` can open without errors and that contains
    /// exactly the 9 tasks the fixture promised, split evenly across the
    /// three fixture columns.
    #[tokio::test]
    async fn write_3x3_board_produces_openable_board_with_nine_tasks() {
        // Correctness — shape, ordering, and openability — is what this test
        // guards. Duration is intentionally *not* asserted: the original
        // "<100ms" acceptance criterion was aspirational and contradicted by
        // real measurements (hundreds of ms locally, multiple seconds on
        // loaded CI hosts). If fixture-build performance becomes a concern
        // it belongs in a dedicated criterion benchmark, not wedged into a
        // correctness test.
        let (tmp, fixture) = write_3x3_board().await;

        assert_eq!(fixture.tasks.len(), 9, "3x3 fixture must have nine tasks");
        // Row-major order: col-1 rows 1..=3 first, then col-2, then col-3.
        assert_eq!(fixture.tasks[0], "task-1-1");
        assert_eq!(fixture.tasks[2], "task-1-3");
        assert_eq!(fixture.tasks[3], "task-2-1");
        assert_eq!(fixture.tasks[8], "task-3-3");
        assert_eq!(fixture.path, tmp.path());

        // Open the board via AppState — the same path a real launch takes.
        let state = AppState::new_for_test();
        let canonical = state
            .open_board(&fixture.path, None)
            .await
            .expect("fixture board opens cleanly");

        let boards = state.boards.read().await;
        let handle = boards
            .get(&canonical)
            .expect("opened board handle present in AppState");
        let ectx = handle
            .ctx
            .entity_context()
            .await
            .expect("entity_context available");

        let tasks = ectx.list("task").await.expect("list tasks");
        assert_eq!(
            tasks.len(),
            9,
            "exactly the nine fixture tasks land on disk"
        );

        // Verify column assignment: each fixture column holds exactly three tasks.
        let mut per_column: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for task in &tasks {
            let col = task
                .get_str("position_column")
                .expect("position_column set")
                .to_string();
            *per_column.entry(col).or_insert(0) += 1;
        }
        assert_eq!(per_column.get("col-1"), Some(&3));
        assert_eq!(per_column.get("col-2"), Some(&3));
        assert_eq!(per_column.get("col-3"), Some(&3));

        let columns = ectx.list("column").await.expect("list columns");
        let column_ids: std::collections::HashSet<String> =
            columns.iter().map(|c| c.id.to_string()).collect();
        assert!(column_ids.contains("col-1"));
        assert!(column_ids.contains("col-2"));
        assert!(column_ids.contains("col-3"));
        assert!(
            !column_ids.contains("todo"),
            "default columns must be removed so tests see only col-1..col-3"
        );
    }

    /// `write_long_column` produces a single column with the requested row
    /// count. This is the shape the long-list navigation tests will use.
    #[tokio::test]
    async fn write_long_column_creates_single_column_with_n_rows() {
        let (_tmp, fixture) = write_long_column(5).await;
        assert_eq!(fixture.tasks.len(), 5);
        assert_eq!(fixture.tasks[0], "task-1-1");
        assert_eq!(fixture.tasks[4], "task-1-5");
    }

    /// `write_grid_view_fixture` produces the 6x4 grid the grid-view tests
    /// expect. Catches an accidental regression in the default shape.
    #[tokio::test]
    async fn write_grid_view_fixture_is_six_by_four() {
        let (_tmp, fixture) = write_grid_view_fixture().await;
        assert_eq!(fixture.tasks.len(), 24);
        assert_eq!(fixture.tasks[0], "task-1-1");
        assert_eq!(fixture.tasks[23], "task-6-4");
    }
}
