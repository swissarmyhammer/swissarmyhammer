//! Shared test helpers for the `entity` MCP server end-to-end tests.
//!
//! Mints an rmcp `Peer<RoleServer>` against a closed transport so tests can
//! build a real `RequestContext` and drive `EntityServer::call_tool` without
//! spinning up a full transport pair. Also wires a complete entity kernel —
//! `EntityContext` + a shared `StoreContext` + `EntityTypeStore` handles for
//! the `tag` and `task` types — exactly as production
//! (`swissarmyhammer-kanban`'s `wire_store_substrate`) does, so undo and
//! events behave the same way in tests.

#![allow(dead_code)] // shared by multiple test modules

use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, CallToolResult, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use swissarmyhammer_entity::test_utils::test_fields_context;
use swissarmyhammer_entity::{EntityContext, EntityTypeStore};
use swissarmyhammer_entity_mcp::EntityServer;
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::clipboard::InMemoryClipboard;
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor, OperationProcessor};
use swissarmyhammer_store::{StoreContext, StoreHandle};
use swissarmyhammer_ui_state::UIState;
use tempfile::TempDir;

/// A transport that yields no messages and closes immediately, used solely
/// to mint a `Peer<RoleServer>` for the `RequestContext` an rmcp call needs.
struct ClosedTransport;

impl Transport<RoleServer> for ClosedTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        _item: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        std::future::ready(Ok(()))
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleServer>>> + Send {
        std::future::ready(None)
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

/// Mint an inert `Peer<RoleServer>` by briefly serving a placeholder handler
/// over a closed transport.
fn mint_peer() -> Peer<RoleServer> {
    struct PeerProbe;
    impl ServerHandler for PeerProbe {}

    let running = serve_directly(PeerProbe, ClosedTransport, None);
    running.peer().clone()
}

/// Build a default `RequestContext` for the `entity` server. The server's
/// verb handlers do not read anything out of the context, but the rmcp
/// signature still requires one.
pub fn request_context() -> RequestContext<RoleServer> {
    RequestContext::new(NumberOrString::Number(0), mint_peer())
}

/// A fully wired entity kernel and its shared substrate, kept alive for the
/// duration of a test.
///
/// Holds the `TempDir` so the storage root is not reclaimed mid-test, the
/// shared `Arc<StoreContext>` (so tests can drive `undo` directly), and the
/// `Arc<EntityContext>` kernel the `EntityServer` dispatches against.
pub struct Harness {
    pub dir: TempDir,
    pub store_ctx: Arc<StoreContext>,
    pub entity_ctx: Arc<EntityContext>,
}

impl Harness {
    /// Build a kernel wired for the `tag` (plain YAML) and `task`
    /// (frontmatter + body) entity types from [`test_fields_context`].
    ///
    /// Mirrors `swissarmyhammer-kanban::substrate::register_entity_stores`:
    /// one `StoreContext`, an `EntityTypeStore` per type registered with both
    /// the kernel (`register_store`) and the shared context (`register`), and
    /// `set_store_context` so writes push onto the undo stack.
    pub async fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let entity_ctx = Arc::new(EntityContext::new(dir.path(), fields.clone()));

        let store_ctx = Arc::new(StoreContext::new(dir.path().to_path_buf()));
        entity_ctx.set_store_context(Arc::clone(&store_ctx));

        for entity_type in ["tag", "task"] {
            let entity_dir = entity_ctx.entity_dir(entity_type);
            std::fs::create_dir_all(&entity_dir).unwrap();

            let entity_def = fields.get_entity(entity_type).unwrap();
            let field_defs: Vec<_> = fields
                .fields_for_entity(entity_type)
                .into_iter()
                .cloned()
                .collect();

            let store = EntityTypeStore::new(
                &entity_dir,
                entity_type,
                Arc::new(entity_def.clone()),
                Arc::new(field_defs),
            );
            let handle = Arc::new(StoreHandle::new(Arc::new(store)));
            entity_ctx.register_store(entity_type, handle.clone()).await;
            store_ctx.register(handle).await;
        }

        Self {
            dir,
            store_ctx,
            entity_ctx,
        }
    }

    /// Build an `EntityServer` over the harness's shared kernel.
    pub fn server(&self) -> EntityServer {
        EntityServer::new(Arc::clone(&self.entity_ctx))
    }
}

/// A full board-backed harness for the clipboard ops.
///
/// The bare [`Harness`] wires a minimal `tag`/`task` kernel from
/// `test_fields_context`, which is enough for CRUD but not for paste — the
/// paste handlers need real columns, the kanban field definitions, and a
/// `KanbanContext` to dispatch sub-ops against. This harness stands up the
/// production board substrate exactly the way `swissarmyhammer-kanban`'s
/// `wire_store_substrate` does: a `KanbanContext` with an initialized board
/// and one shared `Arc<StoreContext>` wired into the kernel, so writes made
/// by the paste handlers push onto the *same* undo stack the test drives.
///
/// The `EntityServer` it builds shares that `KanbanContext`'s
/// `entity_context()` (via [`EntityServer::with_clipboard`]), so an
/// `entity copy/cut/paste` is visible through the generic face and
/// reversible via `store_ctx.undo()`.
pub struct ClipboardHarness {
    pub dir: TempDir,
    pub store_ctx: Arc<StoreContext>,
    pub kanban: Arc<KanbanContext>,
    pub entity_ctx: Arc<EntityContext>,
    pub clipboard: Arc<InMemoryClipboard>,
    pub ui_state: Arc<UIState>,
}

impl ClipboardHarness {
    /// Stand up a board, wire the shared store substrate, and return the
    /// harness. Mirrors `wire_store_substrate` so undo behaves like
    /// production.
    pub async fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let kanban = KanbanContext::new(dir.path().join(".kanban"));

        KanbanOperationProcessor::new()
            .process(&InitBoard::new("Clipboard Test"), &kanban)
            .await
            .expect("board init");

        let kanban = Arc::new(kanban);

        // Wire the one shared StoreContext into the kernel exactly as
        // production does, so paste writes are undoable on this stack.
        let store_ctx = swissarmyhammer_kanban::wire_store_substrate(&kanban).await;
        let entity_ctx = kanban.entity_context().await.expect("entity_context");

        let clipboard = Arc::new(InMemoryClipboard::new());
        let ui_state = Arc::new(UIState::new());

        Self {
            dir,
            store_ctx,
            kanban,
            entity_ctx,
            clipboard,
            ui_state,
        }
    }

    /// Build a clipboard-wired `EntityServer` over the harness's shared
    /// board context and in-memory clipboard.
    pub async fn server(&self) -> EntityServer {
        EntityServer::with_clipboard(
            Arc::clone(&self.kanban),
            Arc::clone(&self.clipboard) as Arc<dyn swissarmyhammer_kanban::clipboard::ClipboardProvider>,
            Arc::clone(&self.ui_state),
        )
        .await
        .expect("clipboard-wired server")
    }
}

/// Invoke an `entity` tool verb through the server's `ServerHandler` surface
/// and return the parsed `serde_json::Value` payload on success.
///
/// The `op` parameter is load-bearing in debug builds: it must match
/// `arguments["op"]` so a typo in the call site is caught immediately.
pub async fn call_tool(
    server: &EntityServer,
    op: &str,
    arguments: Value,
) -> Result<Value, McpError> {
    debug_assert_eq!(
        arguments.get("op").and_then(Value::as_str),
        Some(op),
        "call_tool: op parameter must match arguments[\"op\"]",
    );
    let context = request_context();
    let mut request = CallToolRequestParams::new(Cow::Borrowed("entity"));
    if let Value::Object(map) = arguments {
        request = request.with_arguments(map);
    }
    let result = server.call_tool(request, context).await?;
    Ok(extract_structured(&result))
}

/// Pull the `structured_content` payload out of a [`CallToolResult`].
///
/// Every `entity` verb returns a structured response; the helper unwraps the
/// `Option` to keep call sites short.
pub fn extract_structured(result: &CallToolResult) -> Value {
    result
        .structured_content
        .clone()
        .expect("entity tool should return structured content")
}
