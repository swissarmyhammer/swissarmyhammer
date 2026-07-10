//! Default-perspective invariants through the REAL board-open pipeline.
//!
//! Live-bug regression suite for "perspectives gone missing" (task
//! 01KTY6T1GPY94VYWANE9X41SKJ): the default perspective was created by a
//! non-idempotent frontend auto-create racing against a load-once backend
//! cache, accumulating hundreds of duplicate "Default" YAML files — and a
//! stale in-memory perspective cache could present as ZERO perspectives
//! while files existed on disk.
//!
//! Invariants pinned here:
//!
//! 1. Fresh board open → exactly one default perspective.
//! 2. Re-opening is idempotent — still exactly one default.
//! 3. Concurrent opens of the same board converge to one default.
//! 4. A board with zero perspectives recovers its default on open.
//! 5. Duplicate defaults converge to one; user perspectives are preserved.
//! 6. Stale sibling contexts (the multi-process / multi-window case) doing
//!    an `if_absent` ensure-save converge on ONE default file because the
//!    default id is deterministic, not a fresh ULID per create.
//!
//! Every test drives `KanbanContext::open` / the real `perspective.save`
//! command — no raw-insert fixtures.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::commands::perspective_commands::SavePerspectiveCmd;
use swissarmyhammer_kanban::commands_core::{Command, CommandContext};
use swissarmyhammer_kanban::{Execute, KanbanContext};
use tempfile::TempDir;

/// Create a board directory (via the real `InitBoard` op) and return it.
async fn init_board() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);
    InitBoard::new("Recovery Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    (temp, kanban_dir)
}

/// All perspectives visible through a freshly opened context (the same
/// load path the app's board open uses).
async fn perspectives_after_open(kanban_dir: &Path) -> Vec<Value> {
    let ctx = KanbanContext::open(kanban_dir).await.unwrap();
    let pctx = ctx.perspective_context().await.unwrap();
    let pctx = pctx.read().await;
    pctx.all()
        .iter()
        .map(|p| serde_json::to_value(p).unwrap())
        .collect()
}

/// Count the on-disk perspective YAML files whose `name` is "Default".
fn default_files_on_disk(kanban_dir: &Path) -> usize {
    let dir = kanban_dir.join("perspectives");
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap();
        if content.contains("name: Default") {
            count += 1;
        }
    }
    count
}

/// Dispatch `perspective.save` through the real command path.
async fn save_perspective(kanban: &Arc<KanbanContext>, args: HashMap<String, Value>) -> Value {
    let mut cmd_ctx = CommandContext::new("test", vec![], None, args);
    cmd_ctx.set_extension(Arc::clone(kanban));
    SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap()
}

fn args(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

#[tokio::test]
async fn fresh_board_open_creates_exactly_one_default_perspective() {
    let (_temp, kanban_dir) = init_board().await;

    let perspectives = perspectives_after_open(&kanban_dir).await;
    let defaults: Vec<&Value> = perspectives
        .iter()
        .filter(|p| p["name"] == "Default")
        .collect();

    assert_eq!(
        defaults.len(),
        1,
        "fresh board open must create exactly one default perspective, got: {perspectives:?}"
    );
    assert_eq!(defaults[0]["view"], "board");
    assert_eq!(default_files_on_disk(&kanban_dir), 1);
}

#[tokio::test]
async fn ensure_default_is_idempotent_across_reopens() {
    let (_temp, kanban_dir) = init_board().await;

    for _ in 0..3 {
        let _ = perspectives_after_open(&kanban_dir).await;
    }

    let perspectives = perspectives_after_open(&kanban_dir).await;
    assert_eq!(
        perspectives.len(),
        1,
        "repeated board opens must not accumulate perspectives: {perspectives:?}"
    );
    assert_eq!(default_files_on_disk(&kanban_dir), 1);
}

#[tokio::test]
async fn concurrent_board_opens_converge_to_one_default() {
    let (_temp, kanban_dir) = init_board().await;

    // Race 8 simultaneous board opens — each runs the ensure-default
    // recovery against a board that has no perspectives yet.
    let mut handles = Vec::new();
    for _ in 0..8 {
        let dir = kanban_dir.clone();
        handles.push(tokio::spawn(async move {
            let ctx = KanbanContext::open(&dir).await.unwrap();
            let pctx = ctx.perspective_context().await.unwrap();
            let count = pctx.read().await.all().len();
            assert!(
                count >= 1,
                "every open must observe at least one perspective"
            );
        }));
    }
    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(
        default_files_on_disk(&kanban_dir),
        1,
        "concurrent ensure-default must converge to a single default file"
    );
    let perspectives = perspectives_after_open(&kanban_dir).await;
    assert_eq!(perspectives.len(), 1, "{perspectives:?}");
}

#[tokio::test]
async fn board_with_zero_perspectives_recovers_default_on_open() {
    let (_temp, kanban_dir) = init_board().await;

    // First open creates the default.
    let _ = perspectives_after_open(&kanban_dir).await;
    assert_eq!(default_files_on_disk(&kanban_dir), 1);

    // Simulate the observed live state: every perspective file gone.
    let dir = kanban_dir.join("perspectives");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            std::fs::remove_file(path).unwrap();
        }
    }

    // Re-open: the recovery path must recreate the default.
    let perspectives = perspectives_after_open(&kanban_dir).await;
    assert_eq!(
        perspectives.len(),
        1,
        "zero-perspective board must recover a default on open: {perspectives:?}"
    );
    assert_eq!(perspectives[0]["name"], "Default");
    assert_eq!(default_files_on_disk(&kanban_dir), 1);
}

#[tokio::test]
async fn duplicate_defaults_converge_to_one_preserving_user_perspectives() {
    let (_temp, kanban_dir) = init_board().await;

    // Seed duplicates through the REAL save pipeline against a bare
    // (non-reconciling) context — exactly how the duplicates accumulated
    // in production (frontend auto-create racing a stale cache).
    let kanban = Arc::new(KanbanContext::new(&kanban_dir));
    for _ in 0..3 {
        // Legacy shared-by-kind duplicates (no view_id).
        save_perspective(
            &kanban,
            args(&[
                ("name", Value::String("Default".into())),
                ("view", Value::String("board".into())),
            ]),
        )
        .await;
    }
    for _ in 0..2 {
        // Same-view_id duplicates (the post-June-9 live-bug shape).
        save_perspective(
            &kanban,
            args(&[
                ("name", Value::String("Default".into())),
                ("view", Value::String("board".into())),
                ("view_id", Value::String("01JMVIEW0000000000BOARD0".into())),
            ]),
        )
        .await;
    }
    // A user-created perspective that must survive untouched.
    save_perspective(
        &kanban,
        args(&[
            ("name", Value::String("My Filter".into())),
            ("view", Value::String("board".into())),
            ("filter", Value::String("#bug".into())),
        ]),
    )
    .await;
    assert_eq!(default_files_on_disk(&kanban_dir), 5, "seed precondition");

    // The real board-open recovery path must converge the duplicates.
    let perspectives = perspectives_after_open(&kanban_dir).await;
    let defaults: Vec<&Value> = perspectives
        .iter()
        .filter(|p| p["name"] == "Default")
        .collect();
    assert_eq!(
        defaults.len(),
        1,
        "duplicate defaults must converge to exactly one: {perspectives:?}"
    );

    let user: Vec<&Value> = perspectives
        .iter()
        .filter(|p| p["name"] == "My Filter")
        .collect();
    assert_eq!(user.len(), 1, "user perspective must be preserved");
    assert_eq!(
        user[0]["filter"], "#bug",
        "user perspective content must be untouched"
    );
    assert_eq!(default_files_on_disk(&kanban_dir), 1);
}

#[tokio::test]
async fn ensure_save_with_dead_view_id_falls_back_to_kind_scope() {
    let (_temp, kanban_dir) = init_board().await;

    // Open through the real board-open path: views registry loaded,
    // reconciliation has created the one default for the board view.
    let kanban = Arc::new(KanbanContext::open(&kanban_dir).await.unwrap());
    assert_eq!(default_files_on_disk(&kanban_dir), 1, "open precondition");

    // Ensure against a view id that exists in NO registry — the forensic
    // shape (19 Defaults pinned to dead view id "default"). The ensure
    // must fall back to the view-kind scope and converge on the existing
    // default instead of minting `default-no-such-view.yaml`, which the
    // next open would prune (create/prune churn loop).
    let result = save_perspective(
        &kanban,
        args(&[
            ("name", Value::String("Default".into())),
            ("view", Value::String("board".into())),
            ("view_id", Value::String("no-such-view".into())),
            ("if_absent", Value::Bool(true)),
        ]),
    )
    .await;

    assert_ne!(
        result["view_id"], "no-such-view",
        "an ensure must not pin a default to a nonexistent view"
    );
    assert!(
        !kanban_dir
            .join("perspectives")
            .join("default-no-such-view.yaml")
            .exists(),
        "no default file keyed by the dead view id may be minted"
    );
    assert_eq!(
        default_files_on_disk(&kanban_dir),
        1,
        "the ensure must converge on the existing default"
    );
}

#[tokio::test]
async fn ensure_save_with_path_separator_view_id_cannot_escape_perspectives_dir() {
    let (_temp, kanban_dir) = init_board().await;

    // A bare context (no views registry wired) — existence validation is
    // impossible, so the filename-safety check is the backstop: a scope
    // component with path separators must never reach the on-disk
    // `default-<scope>.yaml` filename.
    let kanban = Arc::new(KanbanContext::new(&kanban_dir));
    let result = save_perspective(
        &kanban,
        args(&[
            ("name", Value::String("Default".into())),
            ("view", Value::String("board".into())),
            ("view_id", Value::String("../escape".into())),
            ("if_absent", Value::Bool(true)),
        ]),
    )
    .await;

    assert_eq!(
        result["id"], "default-board",
        "unsafe view_id must fall back to the kind-scope deterministic id"
    );
    assert!(
        kanban_dir
            .join("perspectives")
            .join("default-board.yaml")
            .exists(),
        "the default must land inside the perspectives dir"
    );
    assert!(
        !kanban_dir.join("escape.yaml").exists()
            && !kanban_dir.join("perspectives").join("escape.yaml").exists(),
        "a path-separator view_id must not escape the perspectives dir"
    );
}

#[tokio::test]
async fn ensure_save_with_overlong_view_id_falls_back_to_kind_scope() {
    let (_temp, kanban_dir) = init_board().await;

    let kanban = Arc::new(KanbanContext::new(&kanban_dir));
    // 300 otherwise-safe chars: well past any real view id (ULIDs are 26)
    // and past the filename-length bound for the scope component.
    let overlong = "v".repeat(300);
    let result = save_perspective(
        &kanban,
        args(&[
            ("name", Value::String("Default".into())),
            ("view", Value::String("board".into())),
            ("view_id", Value::String(overlong.clone())),
            ("if_absent", Value::Bool(true)),
        ]),
    )
    .await;

    assert_eq!(
        result["id"], "default-board",
        "an overlong view_id must fall back to the kind-scope deterministic id"
    );
    assert!(
        !kanban_dir
            .join("perspectives")
            .join(format!("default-{overlong}.yaml"))
            .exists(),
        "no default file keyed by the overlong view id may be minted"
    );
}

#[tokio::test]
async fn stale_context_ensure_save_converges_to_single_default_file() {
    let (_temp, kanban_dir) = init_board().await;

    // Two sibling contexts on the same board — the multi-window /
    // multi-process case. Both load their perspective cache while the
    // board has no perspectives.
    let ctx_a = Arc::new(KanbanContext::new(&kanban_dir));
    let ctx_b = Arc::new(KanbanContext::new(&kanban_dir));
    ctx_a.perspective_context().await.unwrap();
    ctx_b.perspective_context().await.unwrap();

    let ensure_args = || {
        args(&[
            ("name", Value::String("Default".into())),
            ("view", Value::String("board".into())),
            ("if_absent", Value::Bool(true)),
        ])
    };

    // A creates the default.
    let a = save_perspective(&ctx_a, ensure_args()).await;
    // B's cache is now STALE (it never sees A's write — there is no
    // perspective file watcher). Its ensure-save must still converge on
    // the same file instead of minting a second default.
    let b = save_perspective(&ctx_b, ensure_args()).await;

    assert_eq!(
        a["id"], b["id"],
        "ensure-created defaults must share a deterministic id"
    );
    assert_eq!(
        default_files_on_disk(&kanban_dir),
        1,
        "stale-cache ensure must not create a second default file"
    );

    let perspectives = perspectives_after_open(&kanban_dir).await;
    let defaults: Vec<&Value> = perspectives
        .iter()
        .filter(|p| p["name"] == "Default")
        .collect();
    assert_eq!(defaults.len(), 1, "{perspectives:?}");
}
