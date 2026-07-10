//! Cross-context perspective staleness regression (task 01KTYE4VCQ33KWH493WZN7C7V9).
//!
//! Two `KanbanContext`s share one `.kanban` directory — the multi-window /
//! multi-process case. When context A renames or deletes a perspective on
//! disk, context B (which loaded its `PerspectiveContext` once and started a
//! file watcher) must converge its in-memory perspective list WITHOUT
//! re-opening the board.
//!
//! Before the fix, the entity watcher routed `.kanban/perspectives/<id>.yaml`
//! events through `EntityCache::refresh_from_disk_with("perspective", ...)`,
//! which failed with `UnknownEntityType` and dropped the event — so B's
//! `perspective.list` stayed stale forever. The fix adds a dedicated watcher
//! route that drives `PerspectiveContext::reload_from_disk_with`, which both
//! refreshes the in-memory cache and broadcasts on the perspective event bus
//! (the same bus the frontend tab bar refetches from).
//!
//! Real-pipeline only: a real `InitBoard`, real `KanbanContext::open`, a real
//! `EntityWatcher` started via `start_watcher`, real on-disk writes through a
//! sibling `PerspectiveContext`, and the real `ListPerspectives` command on
//! the watched context.

use std::time::Duration;

use serde_json::Value;
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::perspective::{ListPerspectives, Perspective};
use swissarmyhammer_kanban::{Execute, KanbanContext};
use swissarmyhammer_perspectives::PerspectiveEvent;
use tempfile::TempDir;

/// Create a board directory (via the real `InitBoard` op) and return it.
async fn init_board() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(&kanban_dir);
    InitBoard::new("Watcher Test")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    (temp, kanban_dir)
}

/// Run the real `ListPerspectives` command against `ctx` and return the
/// `name` of every perspective the command reports.
async fn list_names(ctx: &KanbanContext) -> Vec<String> {
    let value: Value = ListPerspectives::new()
        .execute(ctx)
        .await
        .into_result()
        .unwrap();
    value["perspectives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

/// Poll `ListPerspectives` on `ctx` until `pred` holds or the deadline passes,
/// re-applying `retrigger` each iteration.
///
/// The `notify` watcher exposes no "ready" signal and macOS FSEvents delivery
/// is asynchronous, so we poll rather than sleep a fixed interval. `retrigger`
/// re-applies the on-disk mutation each iteration: once the watch is live a
/// fresh event reliably fires, mirroring the warm-up-resilient pattern already
/// used by the attachment watcher integration tests.
async fn wait_until<F, R, Fut>(ctx: &KanbanContext, mut retrigger: R, pred: F) -> Vec<String>
where
    F: Fn(&[String]) -> bool,
    R: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let names = list_names(ctx).await;
        if pred(&names) {
            return names;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("perspective list never converged; last seen: {names:?}");
        }
        retrigger().await;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

/// A renamed perspective on disk (written by sibling context A) must surface
/// in context B's `perspective.list` without B re-opening the board.
#[tokio::test]
async fn external_rename_reaches_watched_context_list() {
    let (_temp, kanban_dir) = init_board().await;

    // Seed a user perspective via context A's real PerspectiveContext write.
    let ctx_a = KanbanContext::open(&kanban_dir).await.unwrap();
    {
        let pctx = ctx_a.perspective_context().await.unwrap();
        let mut pctx = pctx.write().await;
        pctx.write(&Perspective::new(
            "01PWATCHRENAME000000000000",
            "Before",
            "board",
        ))
        .await
        .unwrap();
    }

    // Context B opens the SAME board and starts its watcher.
    let ctx_b = KanbanContext::open(&kanban_dir).await.unwrap();
    ctx_b.entity_context().await.unwrap();
    ctx_b.perspective_context().await.unwrap();
    assert!(ctx_b.start_watcher().unwrap(), "watcher must start");

    // B's list initially has the seeded "Before" perspective.
    let before = list_names(&ctx_b).await;
    assert!(
        before.iter().any(|n| n == "Before"),
        "B must see the seeded perspective before the rename: {before:?}"
    );

    // A renames the perspective on disk (real PerspectiveContext write).
    let rename = || {
        let ctx_a = &ctx_a;
        async move {
            let pctx = ctx_a.perspective_context().await.unwrap();
            let mut pctx = pctx.write().await;
            let _ = pctx.rename("01PWATCHRENAME000000000000", "After").await;
        }
    };
    rename().await;

    // B's watcher-driven list must converge to the new name.
    let after = wait_until(&ctx_b, rename, |names| names.iter().any(|n| n == "After")).await;
    assert!(
        after.iter().any(|n| n == "After"),
        "B's perspective.list must reflect the external rename: {after:?}"
    );
    assert!(
        !after.iter().any(|n| n == "Before"),
        "the old name must be gone after the rename: {after:?}"
    );
}

/// A deleted perspective on disk (removed by sibling context A) must vanish
/// from context B's `perspective.list` without B re-opening the board.
#[tokio::test]
async fn external_delete_reaches_watched_context_list() {
    let (_temp, kanban_dir) = init_board().await;

    let ctx_a = KanbanContext::open(&kanban_dir).await.unwrap();
    {
        let pctx = ctx_a.perspective_context().await.unwrap();
        let mut pctx = pctx.write().await;
        pctx.write(&Perspective::new(
            "01PWATCHDELETE000000000000",
            "Doomed",
            "board",
        ))
        .await
        .unwrap();
    }

    let ctx_b = KanbanContext::open(&kanban_dir).await.unwrap();
    ctx_b.entity_context().await.unwrap();
    ctx_b.perspective_context().await.unwrap();
    assert!(ctx_b.start_watcher().unwrap(), "watcher must start");

    let before = list_names(&ctx_b).await;
    assert!(
        before.iter().any(|n| n == "Doomed"),
        "B must see the seeded perspective before the delete: {before:?}"
    );

    // A deletes the perspective on disk.
    let delete = || {
        let ctx_a = &ctx_a;
        async move {
            let pctx = ctx_a.perspective_context().await.unwrap();
            let mut pctx = pctx.write().await;
            let _ = pctx.delete("01PWATCHDELETE000000000000").await;
        }
    };
    delete().await;

    let after = wait_until(&ctx_b, delete, |names| !names.iter().any(|n| n == "Doomed")).await;
    assert!(
        !after.iter().any(|n| n == "Doomed"),
        "B's perspective.list must drop the externally deleted perspective: {after:?}"
    );
}

/// The watcher-driven reload must broadcast on the perspective event bus — the
/// SAME bus `notify_fanin` forwards to `notifications/store/changed` so the
/// frontend tab bar refetches (the store-event-loop guarantee). This asserts
/// the loop is satisfied end to end, not just that the in-memory list mutated.
#[tokio::test]
async fn external_change_broadcasts_on_perspective_bus() {
    let (_temp, kanban_dir) = init_board().await;

    let ctx_a = KanbanContext::open(&kanban_dir).await.unwrap();
    {
        let pctx = ctx_a.perspective_context().await.unwrap();
        let mut pctx = pctx.write().await;
        pctx.write(&Perspective::new(
            "01PWATCHEVENT0000000000000",
            "Watched",
            "board",
        ))
        .await
        .unwrap();
    }

    let ctx_b = KanbanContext::open(&kanban_dir).await.unwrap();
    ctx_b.entity_context().await.unwrap();

    // Subscribe to B's perspective bus BEFORE starting the watcher so no
    // event can slip past the subscription.
    let mut bus = {
        let pctx = ctx_b.perspective_context().await.unwrap();
        let pctx = pctx.read().await;
        pctx.subscribe()
    };
    assert!(ctx_b.start_watcher().unwrap(), "watcher must start");

    // A modifies the perspective on disk.
    let rename = || {
        let ctx_a = &ctx_a;
        async move {
            let pctx = ctx_a.perspective_context().await.unwrap();
            let mut pctx = pctx.write().await;
            let _ = pctx.rename("01PWATCHEVENT0000000000000", "Watched-2").await;
        }
    };
    rename().await;

    // B's watcher must broadcast a PerspectiveChanged for the watched id.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        match tokio::time::timeout(Duration::from_millis(200), bus.recv()).await {
            Ok(Ok(PerspectiveEvent::PerspectiveChanged { id, origin, .. }))
                if id == "01PWATCHEVENT0000000000000" =>
            {
                // Watcher-sourced reloads stamp the `watcher` origin.
                assert_eq!(
                    origin, "watcher",
                    "watcher reload must stamp watcher origin"
                );
                return;
            }
            Ok(Ok(_)) => continue,
            Ok(Err(_)) | Err(_) => {
                if tokio::time::Instant::now() >= deadline {
                    panic!("no PerspectiveChanged broadcast from the watcher within 10s");
                }
                rename().await;
            }
        }
    }
}
