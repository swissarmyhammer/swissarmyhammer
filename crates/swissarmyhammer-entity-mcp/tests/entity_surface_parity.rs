//! Guards that the generic `entity` MCP face is purely additive over the
//! shared `EntityContext` kernel and does not disturb the domain `kanban`
//! face.
//!
//! Two concerns live here:
//!
//! 1. **kanban-surface-frozen guard** — the `kanban` tool's operation surface
//!    (the noun->verb->op `_meta` tree generated from `kanban_operations()`,
//!    plus the flat `op` enum) is byte-for-byte the set this test pins. The
//!    `entity` server work is additive: it adds a *separate* `entity` tool and
//!    must never add, remove, or rename a `kanban` op. If the kanban surface
//!    drifts, this test fails and the change must be made deliberately.
//!
//! 2. **parity** — `kanban add task` and `entity AddEntity{type:"task"}` both
//!    resolve through the one `EntityContext` kernel, so a task created either
//!    way lands in the same `tasks/` directory in the same on-disk format, and
//!    the generic `entity` face can read back a task the domain face created.

use std::borrow::Cow;
use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServerHandler};
use serde_json::{json, Value};
use swissarmyhammer_entity::EntityTypeStore;
use swissarmyhammer_entity_mcp::EntityServer;
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::schema::kanban_operations;
use swissarmyhammer_kanban::task::AddTask;
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor, OperationProcessor};
use swissarmyhammer_operations::generate_operations_meta;
use swissarmyhammer_store::StoreHandle;
use tempfile::TempDir;

// =============================================================================
// kanban-surface-frozen guard
// =============================================================================

/// The complete, ordered list of `(op-string)` the `kanban` tool publishes.
///
/// This is the frozen contract. The `entity` server is additive; it must not
/// change this set. Adding/removing/renaming a kanban op is the only thing
/// that should ever edit this list, and only deliberately.
const FROZEN_KANBAN_OPS: &[&str] = &[
    // Board
    "init board",
    "get board",
    "update board",
    // Column
    "add column",
    "get column",
    "update column",
    "delete column",
    "list columns",
    // Actor
    "add actor",
    "get actor",
    "update actor",
    "delete actor",
    "list actors",
    // Task
    "add task",
    "get task",
    "update task",
    "delete task",
    "move task",
    "complete task",
    "assign task",
    "unassign task",
    "next task",
    "tag task",
    "untag task",
    "list tasks",
    "archive task",
    "unarchive task",
    "list archived",
    // Tag
    "add tag",
    "get tag",
    "update tag",
    "delete tag",
    "list tags",
    // Attachment
    "add attachment",
    "get attachment",
    "update attachment",
    "delete attachment",
    "list attachments",
    // Project
    "add project",
    "get project",
    "update project",
    "delete project",
    "list projects",
    // Perspective
    "add perspective",
    "get perspective",
    "update perspective",
    "delete perspective",
    "list perspectives",
];

/// The kanban tool's op surface is exactly the frozen set, in order. Pins the
/// flat list `kanban_operations()` produces so additive work elsewhere (the
/// `entity` server) cannot silently reshape the domain face.
#[test]
fn kanban_op_surface_is_frozen() {
    let ops = kanban_operations();
    let actual: Vec<String> = ops.iter().map(|op| op.op_string()).collect();
    let expected: Vec<String> = FROZEN_KANBAN_OPS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        actual, expected,
        "kanban operation surface drifted — this work must not add/remove/rename a kanban op"
    );
}

/// The kanban `_meta` operations tree carries exactly the frozen op set. Pins
/// the discovery surface (noun->verb->{op}) the same way the wire `op` enum is
/// pinned above, so a drift in either representation is caught.
#[test]
fn kanban_meta_operations_tree_is_frozen() {
    let meta = generate_operations_meta(kanban_operations());

    // Collect every leaf `op` string from the noun->verb->{op} tree.
    let mut tree_ops: Vec<String> = Vec::new();
    let tree = meta.as_object().expect("meta is an object");
    for verbs in tree.values() {
        let verbs = verbs.as_object().expect("noun maps to a verb object");
        for leaf in verbs.values() {
            if let Some(op) = leaf.get("op").and_then(Value::as_str) {
                tree_ops.push(op.to_string());
            }
        }
    }
    tree_ops.sort();

    let mut expected: Vec<String> = FROZEN_KANBAN_OPS.iter().map(|s| s.to_string()).collect();
    expected.sort();

    assert_eq!(
        tree_ops, expected,
        "kanban _meta operations tree drifted from the frozen op set"
    );
}

// =============================================================================
// parity: kanban add task ≡ entity AddEntity{type:task}
// =============================================================================

/// A transport that yields no messages and closes immediately, used solely to
/// mint a `Peer<RoleServer>` for the `RequestContext` an rmcp call needs.
struct ClosedTransport;

impl Transport<RoleServer> for ClosedTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        _item: TxJsonRpcMessage<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        std::future::ready(Ok(()))
    }

    fn receive(
        &mut self,
    ) -> impl std::future::Future<Output = Option<RxJsonRpcMessage<RoleServer>>> + Send {
        std::future::ready(None)
    }

    fn close(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

/// Mint an inert `Peer<RoleServer>`.
fn mint_peer() -> Peer<RoleServer> {
    struct PeerProbe;
    impl ServerHandler for PeerProbe {}
    serve_directly(PeerProbe, ClosedTransport, None)
        .peer()
        .clone()
}

/// Build a `KanbanContext` with an initialized board and `EntityTypeStore`
/// handles registered for every entity type — the production-like path used
/// by `command_snapshots.rs`.
async fn kanban_with_board() -> (TempDir, Arc<KanbanContext>) {
    let temp = TempDir::new().unwrap();
    let kanban = KanbanContext::new(temp.path().join(".kanban"));

    KanbanOperationProcessor::new()
        .process(&InitBoard::new("Parity Test"), &kanban)
        .await
        .expect("board init");

    let kanban = Arc::new(kanban);

    let ectx = kanban.entity_context().await.expect("entity_context");
    let fields_ctx = ectx.fields();
    for entity_def in fields_ctx.all_entities() {
        let entity_type = entity_def.name.as_str();
        let field_defs: Vec<_> = fields_ctx
            .fields_for_entity(entity_type)
            .into_iter()
            .cloned()
            .collect();
        let store = EntityTypeStore::new(
            ectx.entity_dir(entity_type),
            entity_type,
            Arc::new(entity_def.clone()),
            Arc::new(field_defs),
        );
        let handle = Arc::new(StoreHandle::new(Arc::new(store)));
        ectx.register_store(entity_type, handle).await;
    }

    (temp, kanban)
}

/// Invoke an `entity` verb against the kernel-backed server.
///
/// `op` is load-bearing in debug builds: it must match `args["op"]` so a typo
/// in the call site is caught immediately.
async fn entity_call(server: &EntityServer, op: &str, args: Value) -> Value {
    debug_assert_eq!(
        args.get("op").and_then(Value::as_str),
        Some(op),
        "entity_call: op parameter must match args[\"op\"]",
    );
    let context = RequestContext::new(NumberOrString::Number(0), mint_peer());
    let mut request = CallToolRequestParams::new(Cow::Borrowed("entity"));
    if let Value::Object(map) = args {
        request = request.with_arguments(map);
    }
    let result = server
        .call_tool(request, context)
        .await
        .expect("entity call");
    result
        .structured_content
        .expect("entity tool returns structured content")
}

/// A task added via `kanban add task` and one added via
/// `entity AddEntity{type:"task"}` resolve through the one kernel: both write
/// a `.md` file into the same `tasks/` directory, and the generic `entity`
/// face reads back the kanban-created task with the title the domain face set.
#[tokio::test]
async fn kanban_add_task_and_entity_add_entity_share_the_kernel() {
    let (temp, kanban) = kanban_with_board().await;
    let ectx = kanban.entity_context().await.unwrap();
    let server = EntityServer::new(Arc::clone(&ectx));

    // 1. Domain face: add a task through the kanban processor.
    let processor = KanbanOperationProcessor::new();
    processor
        .process(&AddTask::new("From kanban"), &kanban)
        .await
        .expect("kanban add task");

    // 2. Generic face: add a task through the entity server.
    let added = entity_call(
        &server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "task",
            "fields": { "title": "From entity", "body": "" },
        }),
    )
    .await;
    let entity_task_id = added["id"].as_str().expect("minted id").to_string();

    // Both tasks live in the same directory, in the same `.md` format.
    let tasks_dir = temp.path().join(".kanban/tasks");
    let md_files: Vec<_> = std::fs::read_dir(&tasks_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .collect();
    assert_eq!(
        md_files.len(),
        2,
        "both faces wrote a .md task into the shared tasks/ directory"
    );

    // The entity-created task is the one the entity server wrote.
    assert!(
        tasks_dir.join(format!("{entity_task_id}.md")).exists(),
        "entity-created task file present"
    );

    // The generic face reads back the kanban-created task (proving one
    // kernel): locate it via the kernel's own list, then read it through the
    // entity server.
    let kanban_task = ectx
        .list("task")
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.get_str("title") == Some("From kanban"))
        .expect("kanban task present in the shared kernel");

    let got = entity_call(
        &server,
        "get entity",
        json!({ "op": "get entity", "type": "task", "id": kanban_task.id.to_string() }),
    )
    .await;
    assert_eq!(
        got["entity"]["title"],
        json!("From kanban"),
        "entity face reads back the kanban-created task"
    );
}
