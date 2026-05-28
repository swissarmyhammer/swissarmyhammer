//! End-to-end coverage for the `entity` server's clipboard verbs.
//!
//! These tests drive `entity copy` / `entity cut` / `entity paste` through
//! the real `ServerHandler` surface against a full board substrate, and
//! assert the on-disk effect rather than the command return shape — the
//! point is that the generic face reuses the domain `kanban` clipboard
//! machinery and that its writes flow through the one shared `StoreContext`
//! (so a paste is undoable).
//!
//! The clipboard ops reuse the exact `CopyEntityCmd` / `CutEntityCmd` /
//! `PasteEntityCmd` structs and the shared `PasteMatrix`; there is no
//! duplicate paste logic in this crate. The harness injects an
//! `InMemoryClipboard` as the clipboard seam.

use serde_json::{json, Value};
use swissarmyhammer_kanban::clipboard::deserialize_from_clipboard;

use super::common::{call_tool, ClipboardHarness};

/// List every live task on the harness board.
async fn list_tasks(h: &ClipboardHarness) -> Vec<swissarmyhammer_entity::Entity> {
    h.entity_ctx.list("task").await.expect("list tasks")
}

/// Read a task's enriched attachments list (empty when absent).
async fn read_attachments(h: &ClipboardHarness, task_id: &str) -> Vec<Value> {
    let task = h.entity_ctx.read("task", task_id).await.expect("read task");
    task.get("attachments")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Add a task directly through the kernel-backed kanban processor and
/// return its id. Keeps the clipboard tests focused on copy/cut/paste
/// rather than on the generic `add entity` shape (covered elsewhere).
async fn add_task(h: &ClipboardHarness, title: &str) -> String {
    use swissarmyhammer_kanban::task::AddTask;
    use swissarmyhammer_kanban::{KanbanOperationProcessor, OperationProcessor};
    let result = KanbanOperationProcessor::new()
        .process(&AddTask::new(title), h.kanban.as_ref())
        .await
        .expect("add task");
    result["id"].as_str().expect("task id").to_string()
}

// =============================================================================
// copy → paste → duplicate
// =============================================================================

/// Copying a task and pasting it into a different column creates a brand
/// new task on disk — a duplicate with a fresh id — while the source is
/// left intact. This exercises the `(task, column)` paste handler from the
/// shared `PasteMatrix` end-to-end through the MCP surface.
#[tokio::test]
async fn copy_then_paste_creates_a_duplicate_on_disk() {
    let h = ClipboardHarness::new().await;
    let server = h.server().await;

    let source_id = add_task(&h, "Source task").await;
    assert_eq!(list_tasks(&h).await.len(), 1);

    // Copy the source via the generic `entity copy` verb.
    let copied = call_tool(
        &server,
        "copy entity",
        json!({ "op": "copy entity", "type": "task", "id": source_id }),
    )
    .await
    .expect("copy entity");
    assert_eq!(copied["copied"], true);
    assert_eq!(copied["entity_type"], "task");

    // The injected clipboard now carries the snapshot.
    let clip_text = h.clipboard.peek().expect("clipboard populated by copy");
    let payload = deserialize_from_clipboard(&clip_text).expect("payload roundtrips");
    assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "task");
    assert_eq!(payload.swissarmyhammer_clipboard.mode, "copy");

    // Paste into the `doing` column.
    let pasted = call_tool(
        &server,
        "paste entity",
        json!({ "op": "paste entity", "target": "column:doing" }),
    )
    .await
    .expect("paste entity");
    let new_id = pasted["id"].as_str().expect("pasted task has an id");
    assert_ne!(new_id, source_id, "paste must mint a fresh id");

    // Two tasks on disk: source preserved, duplicate created.
    let tasks = list_tasks(&h).await;
    assert_eq!(tasks.len(), 2, "copy+paste must duplicate the task");
    assert!(
        tasks.iter().any(|t| t.id == source_id),
        "source task must survive a copy"
    );
    assert!(
        tasks.iter().any(|t| t.id.as_str() == new_id),
        "the new duplicate must be on disk"
    );
}

// =============================================================================
// cut → paste → move (attachment between tasks)
// =============================================================================

/// Cut an attachment from task A and paste it onto task B: the source loses
/// it and the destination gains it — the "move" the user sees. This is the
/// canonical cut/paste move path (a task's column move is internal drag, a
/// property mutation handled elsewhere — not the paste path). It drives the
/// `entity cut` verb (snapshot + stage + destructive detach) and the
/// `(attachment, task)` paste handler from the shared `PasteMatrix`.
#[tokio::test]
async fn cut_then_paste_moves_an_attachment_between_tasks() {
    use swissarmyhammer_kanban::attachment::AddAttachment;
    use swissarmyhammer_kanban::{KanbanOperationProcessor, OperationProcessor};

    let h = ClipboardHarness::new().await;
    let server = h.server().await;

    let source_id = add_task(&h, "Source").await;
    let dest_id = add_task(&h, "Destination").await;

    // Attach a file to the source task.
    let file = h.dir.path().join("diagram.png");
    std::fs::write(&file, b"diagram bytes").unwrap();
    KanbanOperationProcessor::new()
        .process(
            &AddAttachment::new(source_id.as_str(), "diagram.png", file.to_str().unwrap()),
            h.kanban.as_ref(),
        )
        .await
        .expect("add attachment");

    let source_attachments = read_attachments(&h, &source_id).await;
    assert_eq!(source_attachments.len(), 1);
    let attachment_path = source_attachments[0]["path"].as_str().unwrap().to_string();

    // Cut the attachment. The cut needs the owning `task:` moniker in scope
    // so the destructive detach can find the parent.
    let cut = call_tool(
        &server,
        "cut entity",
        json!({
            "op": "cut entity",
            "type": "attachment",
            "id": attachment_path,
            "scope": [
                format!("attachment:{attachment_path}"),
                format!("task:{source_id}"),
                "column:todo",
            ],
        }),
    )
    .await
    .expect("cut entity");
    assert_eq!(cut["cut"], true);

    // Source has lost the attachment.
    assert!(
        read_attachments(&h, &source_id).await.is_empty(),
        "cut must detach the attachment from the source task"
    );

    // Paste onto the destination task.
    call_tool(
        &server,
        "paste entity",
        json!({ "op": "paste entity", "target": format!("task:{dest_id}") }),
    )
    .await
    .expect("paste entity");

    // Move semantics: source still empty, destination gained the attachment.
    assert!(
        read_attachments(&h, &source_id).await.is_empty(),
        "source must remain empty after the paste (move, not copy)"
    );
    let dest_attachments = read_attachments(&h, &dest_id).await;
    assert_eq!(
        dest_attachments.len(),
        1,
        "destination must have gained the cut attachment"
    );
    assert_eq!(dest_attachments[0]["name"], "diagram.png");
}

// =============================================================================
// undo a paste
// =============================================================================

/// A paste flows through the kernel's shared `StoreContext`, so driving
/// `StoreContext::undo` reverses it. Here a copied task is pasted to create
/// a duplicate; undoing the paste's store entry removes the duplicate,
/// leaving only the source — proving the paste participates in the one undo
/// stack the rest of the app drives rather than a fork of it.
#[tokio::test]
async fn undo_reverts_a_paste() {
    let h = ClipboardHarness::new().await;
    let server = h.server().await;

    let source_id = add_task(&h, "Source").await;

    call_tool(
        &server,
        "copy entity",
        json!({ "op": "copy entity", "type": "task", "id": source_id }),
    )
    .await
    .expect("copy entity");

    let pasted = call_tool(
        &server,
        "paste entity",
        json!({ "op": "paste entity", "target": "column:doing" }),
    )
    .await
    .expect("paste entity");
    let new_id = pasted["id"].as_str().expect("pasted task id").to_string();
    assert_eq!(list_tasks(&h).await.len(), 2, "paste created the duplicate");

    // Undo the most recent store entry — the paste's task creation.
    let outcome = h.store_ctx.undo().await.expect("undo should succeed");
    assert_eq!(
        outcome.store_name, "task",
        "the reverted entry must be the pasted task"
    );

    // The kernel rewrote the data file under the cache; reconcile the cache
    // entry (production does this via the UndoCmd layer) before reading back.
    h.entity_ctx
        .sync_entity_cache_from_disk("task", new_id.as_str())
        .await;

    let tasks = list_tasks(&h).await;
    assert_eq!(
        tasks.len(),
        1,
        "undo must remove the pasted duplicate, leaving only the source"
    );
    assert_eq!(
        tasks[0].id, source_id,
        "the surviving task must be the original source"
    );
}
