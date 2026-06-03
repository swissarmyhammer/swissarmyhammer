//! Shared test helpers for the `window` MCP server end-to-end tests.
//!
//! Provides a recording [`SpyShell`] implementing the `WindowShell` seam, and
//! an rmcp `Peer<RoleServer>` minted against a closed transport so tests can
//! build a real `RequestContext` and drive `WindowService::call_tool` without a
//! live GUI or a real file manager.

#![allow(dead_code)] // shared by multiple test modules

use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use rmcp::model::{CallToolRequestParams, CallToolResult, NumberOrString};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::Value;
use swissarmyhammer_window_service::{
    ContextMenuItem, CreatedBoard, MonitorInfo, NewWindow, OpenedBoard, WindowPosition,
    WindowService, WindowShell,
};

/// A recording [`WindowShell`] used to assert which shell method the service
/// drove for each op, and with what arguments.
///
/// Each call appends a tag describing the call (and its salient argument) to
/// `calls`. Reads return canned data the harness was built with: `open_new_window`
/// returns the canned [`NewWindow`], `get_window_position` returns the canned
/// position for the requested label, and `get_monitors` returns the canned list.
pub struct SpyShell {
    /// Ordered log of shell method tags, one per call.
    pub calls: Mutex<Vec<String>>,
    /// The window `open_new_window` hands back.
    pub new_window: NewWindow,
    /// Per-label canned positions `get_window_position` reads from.
    pub positions: HashMap<String, WindowPosition>,
    /// The monitor list `get_monitors` hands back.
    pub monitors: Vec<MonitorInfo>,
    /// The board `new_board` hands back, simulating the new-board dialog path.
    pub new_board: CreatedBoard,
    /// The board `open_board` hands back, simulating the open-board picker
    /// resolving to a chosen folder. `None` models the user cancelling the
    /// OS file-open dialog.
    pub open_board: Option<OpenedBoard>,
    /// The JSON `list_open_boards` hands back.
    pub open_boards: Value,
    /// The JSON `get_board_data` hands back.
    pub board_data: Value,
    /// The `board_path` argument of the last `get_board_data` call.
    pub last_board_path: Mutex<Option<String>>,
}

impl SpyShell {
    /// Build a spy with canned new-window, positions, and monitor list, and
    /// default board-lifecycle results.
    pub fn new(
        new_window: NewWindow,
        positions: HashMap<String, WindowPosition>,
        monitors: Vec<MonitorInfo>,
    ) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            new_window,
            positions,
            monitors,
            new_board: CreatedBoard {
                path: "/tmp/new-board".to_string(),
                name: "New Board".to_string(),
            },
            open_board: Some(OpenedBoard {
                path: "/tmp/opened-board".to_string(),
            }),
            open_boards: serde_json::json!([]),
            board_data: serde_json::json!({}),
            last_board_path: Mutex::new(None),
        }
    }

    /// Override the board `new_board` hands back (the new-board dialog result).
    pub fn with_new_board(mut self, new_board: CreatedBoard) -> Self {
        self.new_board = new_board;
        self
    }

    /// Override the board `open_board` hands back. `None` models the user
    /// cancelling the OS file-open picker.
    pub fn with_open_board(mut self, open_board: Option<OpenedBoard>) -> Self {
        self.open_board = open_board;
        self
    }

    /// Set the canned values the board-management reads return.
    pub fn with_board_reads(mut self, open_boards: Value, board_data: Value) -> Self {
        self.open_boards = open_boards;
        self.board_data = board_data;
        self
    }

    /// The `board_path` the most recent `get_board_data` call carried.
    pub fn last_board_path(&self) -> Option<String> {
        self.last_board_path.lock().unwrap().clone()
    }

    /// Snapshot the recorded call tags in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, tag: impl Into<String>) {
        self.calls.lock().unwrap().push(tag.into());
    }
}

impl WindowShell for SpyShell {
    fn open_new_window(&self, board_path: Option<String>) -> Result<NewWindow, String> {
        self.record(format!("open_new_window:{board_path:?}"));
        Ok(self.new_window.clone())
    }

    fn activate_window(&self, label: &str) -> Result<(), String> {
        self.record(format!("activate_window:{label}"));
        Ok(())
    }

    fn set_window_position(&self, label: &str, position: WindowPosition) -> Result<(), String> {
        self.record(format!(
            "set_window_position:{label}:{},{}",
            position.x, position.y
        ));
        Ok(())
    }

    fn get_window_position(&self, label: &str) -> Result<WindowPosition, String> {
        self.record(format!("get_window_position:{label}"));
        self.positions
            .get(label)
            .copied()
            .ok_or_else(|| format!("no window with label {label:?}"))
    }

    fn get_monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        self.record("get_monitors");
        Ok(self.monitors.clone())
    }

    fn close_window(&self, label: &str) -> Result<(), String> {
        self.record(format!("close_window:{label}"));
        Ok(())
    }

    fn open_path(&self, path: &str) -> Result<(), String> {
        self.record(format!("open_path:{path}"));
        Ok(())
    }

    fn reveal_path(&self, path: &str) -> Result<(), String> {
        self.record(format!("reveal_path:{path}"));
        Ok(())
    }

    fn switch_board(&self, path: &str) -> Result<(), String> {
        self.record(format!("switch_board:{path}"));
        Ok(())
    }

    fn close_board(&self, path: &str) -> Result<(), String> {
        self.record(format!("close_board:{path}"));
        Ok(())
    }

    fn new_board(&self) -> Result<CreatedBoard, String> {
        self.record("new_board");
        Ok(self.new_board.clone())
    }

    fn open_board(&self) -> Result<Option<OpenedBoard>, String> {
        self.record("open_board");
        Ok(self.open_board.clone())
    }

    fn show_context_menu(
        &self,
        items: Vec<ContextMenuItem>,
        window_label: Option<String>,
    ) -> Result<(), String> {
        // Record the command ids (in order) and the forwarded window label, so
        // tests can assert the service routed both the exact items and the
        // calling window's label to the shell.
        let cmds: Vec<&str> = items.iter().map(|i| i.cmd.as_str()).collect();
        self.record(format!(
            "show_context_menu:[{}]@{}",
            cmds.join(","),
            window_label.as_deref().unwrap_or("-")
        ));
        Ok(())
    }

    fn list_open_boards(&self) -> Result<Value, String> {
        self.record("list_open_boards");
        Ok(self.open_boards.clone())
    }

    fn get_board_data(&self, board_path: Option<String>) -> Result<Value, String> {
        self.record("get_board_data");
        *self.last_board_path.lock().unwrap() = board_path;
        Ok(self.board_data.clone())
    }
}

/// A fully wired `window` service over a recording spy, kept alive for a test.
///
/// Holds the `Arc<SpyShell>` so tests can read back the recorded calls after
/// driving the service.
pub struct Harness {
    /// The shared spy the service routes through.
    pub shell: Arc<SpyShell>,
}

impl Harness {
    /// Build a harness with default canned new-window / positions / monitors.
    pub fn new() -> Self {
        let mut positions = HashMap::new();
        positions.insert("main".to_string(), WindowPosition { x: 10, y: 20 });
        Self::with_shell(SpyShell::new(
            NewWindow {
                label: "board-spy".to_string(),
                board_path: Some("/tmp/board".to_string()),
            },
            positions,
            vec![MonitorInfo {
                name: Some("Built-in Display".to_string()),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale_factor: 2.0,
            }],
        ))
    }

    /// Build a harness around a caller-supplied spy.
    pub fn with_shell(shell: SpyShell) -> Self {
        Self {
            shell: Arc::new(shell),
        }
    }

    /// Build a `WindowService` over the harness's spy shell.
    pub fn service(&self) -> WindowService {
        WindowService::new(Arc::clone(&self.shell) as Arc<dyn WindowShell>)
    }
}

/// A transport that yields no messages and closes immediately, used solely to
/// mint a `Peer<RoleServer>` for the `RequestContext` an rmcp call needs.
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

/// Build a default `RequestContext` for the `window` service. The service's
/// verb handlers do not read anything out of the context, but the rmcp
/// signature still requires one.
pub fn request_context() -> RequestContext<RoleServer> {
    RequestContext::new(NumberOrString::Number(0), mint_peer())
}

/// Invoke a `window` tool verb through the service's `ServerHandler` surface and
/// return the parsed `serde_json::Value` payload on success.
///
/// The `op` parameter is load-bearing in debug builds: it must match
/// `arguments["op"]` so a typo in the call site is caught immediately.
pub async fn call_tool(
    service: &WindowService,
    op: &str,
    arguments: Value,
) -> Result<Value, McpError> {
    debug_assert_eq!(
        arguments.get("op").and_then(Value::as_str),
        Some(op),
        "call_tool: op parameter must match arguments[\"op\"]",
    );
    let context = request_context();
    let mut request = CallToolRequestParams::new(Cow::Borrowed("window"));
    if let Value::Object(map) = arguments {
        request = request.with_arguments(map);
    }
    let result = service.call_tool(request, context).await?;
    Ok(extract_structured(&result))
}

/// Pull the `structured_content` payload out of a [`CallToolResult`].
pub fn extract_structured(result: &CallToolResult) -> Value {
    result
        .structured_content
        .clone()
        .expect("window tool should return structured content")
}
