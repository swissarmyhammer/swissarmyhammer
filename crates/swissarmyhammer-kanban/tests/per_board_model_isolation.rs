//! End-to-end regression test for the per-board model contract.
//!
//! Two independent `.kanban` directories must each remember their own
//! AI-panel model id without leaking state through any shared/global
//! location. This is the disk-round-trip companion to the unit tests in
//! `src/board/update.rs`, which only exercise a single context.
//!
//! If a future change accidentally writes the model to a process-wide
//! static, an XDG-shared file, or any singleton, this test will fail —
//! each board's `board.yaml` would then contain both ids (or the wrong
//! one), breaking the disjoint-content assertions below.

use swissarmyhammer_kanban::{
    board::{GetBoard, InitBoard, UpdateBoard},
    KanbanContext,
};
use swissarmyhammer_operations::Execute;
use tempfile::TempDir;

/// Set up an isolated `.kanban` directory in a fresh `TempDir` and
/// initialize a board with the given name.
///
/// Returns the `TempDir` (held by the caller so the directory survives)
/// and a `KanbanContext` rooted at its `.kanban` subdirectory.
async fn init_board(name: &str) -> (TempDir, KanbanContext) {
    let temp = TempDir::new().expect("tempdir creation must succeed");
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);

    InitBoard::new(name)
        .execute(&ctx)
        .await
        .into_result()
        .unwrap_or_else(|e| panic!("InitBoard({name}) must succeed: {e}"));

    (temp, ctx)
}

/// Two boards with different models persist their ids to disjoint files
/// and do not leak through any shared state.
///
/// The disk-content assertions are the load-bearing part: a regression
/// that routes `UpdateBoard` through a global location would either put
/// both ids into a single shared file or write the wrong id into one of
/// the per-board files, and the `!contains(other_id)` checks would catch
/// both cases.
#[tokio::test]
async fn test_per_board_model_isolation() {
    // Two fully independent boards in two `TempDir`s.
    let (temp_a, ctx_a) = init_board("A").await;
    let (temp_b, ctx_b) = init_board("B").await;

    // Set a different chat-capable model on each board.
    UpdateBoard::new()
        .with_model("claude-code")
        .execute(&ctx_a)
        .await
        .into_result()
        .expect("setting `claude-code` on board A must succeed");

    UpdateBoard::new()
        .with_model("qwen")
        .execute(&ctx_b)
        .await
        .into_result()
        .expect("setting `qwen` on board B must succeed");

    // Each board's raw YAML must contain only its own model id.
    let yaml_a = std::fs::read_to_string(temp_a.path().join(".kanban/boards/board.yaml"))
        .expect("board A's board.yaml must exist after UpdateBoard");
    let yaml_b = std::fs::read_to_string(temp_b.path().join(".kanban/boards/board.yaml"))
        .expect("board B's board.yaml must exist after UpdateBoard");

    assert!(
        yaml_a.contains("model: claude-code"),
        "board A's yaml must contain `model: claude-code`, got:\n{yaml_a}"
    );
    assert!(
        !yaml_a.contains("qwen"),
        "board A's yaml must NOT contain `qwen` (leaked from board B), got:\n{yaml_a}"
    );

    assert!(
        yaml_b.contains("model: qwen"),
        "board B's yaml must contain `model: qwen`, got:\n{yaml_b}"
    );
    assert!(
        !yaml_b.contains("claude-code"),
        "board B's yaml must NOT contain `claude-code` (leaked from board A), got:\n{yaml_b}"
    );

    // GetBoard must read each context's own model id back from disk.
    let board_a = GetBoard::default()
        .execute(&ctx_a)
        .await
        .into_result()
        .expect("GetBoard on context A must succeed");
    let board_b = GetBoard::default()
        .execute(&ctx_b)
        .await
        .into_result()
        .expect("GetBoard on context B must succeed");

    assert_eq!(
        board_a["model"], "claude-code",
        "GetBoard on context A must report `claude-code`"
    );
    assert_eq!(
        board_b["model"], "qwen",
        "GetBoard on context B must report `qwen`"
    );
}
