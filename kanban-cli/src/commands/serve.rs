//! MCP server that exposes the `kanban` operation tool over stdio.
//!
//! Creates a minimal `rmcp` server hosting a single tool named `"kanban"` that
//! dispatches to [`swissarmyhammer_kanban`]. Built directly on the kanban crate
//! so that `kanban-cli` does not depend on `swissarmyhammer-tools`.
//!
//! Error handling follows the same shape as `shelltool-cli`'s `serve` module:
//! fatal errors are surfaced as `Result<(), String>` for the CLI layer to print.

use std::borrow::Cow;
use std::path::PathBuf;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Value};
use swissarmyhammer_kanban::{dispatch, parse::parse_input, schema, KanbanContext, KanbanError};

/// Name advertised as the MCP server implementation. Matches the binary
/// name (`kanban`) so stdio clients that diff or pin server identities see
/// a stable identifier across builds.
const SERVER_NAME: &str = "kanban";

/// Name of the single tool exposed by this server. Aliases `SERVER_NAME`
/// today because a 1:1 server/tool binary is the simplest shape, but they
/// are kept as distinct constants so the two semantic roles can diverge
/// cleanly in the future (e.g. renaming the tool without changing the
/// server identity).
const TOOL_NAME: &str = SERVER_NAME;

/// Short human-readable description for the `kanban` tool. Kept inline so
/// this crate has no dependency on `swissarmyhammer-tools` (which owns the
/// richer Markdown description for the in-process tool).
const TOOL_DESCRIPTION: &str = "Kanban board operations for task management. \
Accepts forgiving input with aliases and inference — use `op` as a \
`\"verb noun\"` string (e.g. `\"add task\"`, `\"move task\"`). \
Operates on the `.kanban` directory in the current working directory.";

/// Minimal MCP server that exposes the `kanban` tool.
///
/// Implements [`rmcp::ServerHandler`] so it can be served directly over
/// stdio using [`rmcp::serve_server`]. The server is stateless: each
/// `call_tool` invocation constructs a fresh [`KanbanContext`] rooted at
/// `<cwd>/.kanban`, matching the behaviour of the schema-driven CLI
/// subcommands in [`crate::main`].
#[derive(Debug, Clone, Default)]
pub struct KanbanMcpServer;

impl KanbanMcpServer {
    /// Create a new `KanbanMcpServer`.
    pub fn new() -> Self {
        Self
    }

    /// Build the `.kanban` directory path for this server.
    ///
    /// Resolves to `<current_working_directory>/.kanban`. The directory is
    /// not required to exist — `KanbanContext` lazily creates files on
    /// first write, and operations like `init board` create the layout.
    fn kanban_dir() -> Result<PathBuf, McpError> {
        let cwd = std::env::current_dir()
            .map_err(|e| McpError::internal_error(format!("cannot read cwd: {e}"), None))?;
        Ok(cwd.join(".kanban"))
    }
}

/// Build the single-tool `ListToolsResult` this server advertises.
///
/// Factored out so unit tests can assert the published shape without
/// needing a live `RequestContext<RoleServer>` — which the rmcp runtime
/// only constructs inside its transport loop.
fn build_list_tools_result() -> ListToolsResult {
    let ops = schema::kanban_operations();
    let schema_value = schema::generate_kanban_mcp_schema(ops);
    let schema_map = match schema_value {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };

    let tool = Tool::new(
        Cow::Borrowed(TOOL_NAME),
        Cow::Borrowed(TOOL_DESCRIPTION),
        schema_map,
    )
    .with_title(TOOL_NAME);

    ListToolsResult {
        tools: vec![tool],
        next_cursor: None,
        meta: None,
    }
}

impl ServerHandler for KanbanMcpServer {
    /// Advertise server capabilities and identity.
    ///
    /// Reports the crate version (`CARGO_PKG_VERSION`) so stdio clients can
    /// pin or diff against a specific `kanban-cli` build.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION")))
    }

    /// List the single `kanban` tool.
    ///
    /// The schema is generated on demand from
    /// [`swissarmyhammer_kanban::schema::kanban_operations`] — the same
    /// source of truth used by the in-process `KanbanTool` in
    /// `swissarmyhammer-tools`.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(build_list_tools_result())
    }

    /// Dispatch a `call_tool` request to the kanban engine.
    ///
    /// Only the `"kanban"` tool is accepted; any other name produces an
    /// `invalid_request` error. Input parsing and execution are delegated
    /// to [`parse_input`] and [`dispatch::execute_operation`] — the same
    /// pipeline used by the noun-verb CLI subcommands.
    ///
    /// The body is a thin wrapper over [`dispatch_call_tool_request`] so
    /// unit tests can exercise the dispatch path without having to
    /// construct a live [`RequestContext`] — which the rmcp runtime only
    /// builds inside its transport loop.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let kanban_dir = Self::kanban_dir()?;
        let ctx = KanbanContext::new(kanban_dir);
        dispatch_call_tool_request(&ctx, request).await
    }
}

/// Execute a `call_tool` request against the given [`KanbanContext`].
///
/// Extracted from [`KanbanMcpServer::call_tool`] so unit tests can drive
/// the dispatch path directly — the `RequestContext<RoleServer>` argument
/// expected by the rmcp trait is only constructible inside the transport
/// runtime, so the handler body has to live in a context-free helper to
/// be testable.
///
/// # Plan notifications
///
/// Note: unlike the in-process `swissarmyhammer-tools::KanbanTool`, this
/// server deliberately drops the ACP `_plan` side channel. The tools-side
/// implementation attaches `session/update` plan data to task-modifying
/// responses so ACP agents can render progress. The CLI MCP server is a
/// stateless stdio binary with no plan sender wired in, so emitting plans
/// here would be a no-op at best and misleading at worst. If plan support
/// ever lands in the CLI, lifting `build_plan_data` from
/// `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` into
/// `swissarmyhammer-kanban` for shared use is the preferred path.
async fn dispatch_call_tool_request(
    ctx: &KanbanContext,
    request: CallToolRequestParams,
) -> Result<CallToolResult, McpError> {
    if request.name != TOOL_NAME {
        return Err(McpError::invalid_request(
            format!("Unknown tool: {}", request.name),
            None,
        ));
    }

    let arguments = request.arguments.unwrap_or_default();
    let input = Value::Object(arguments);
    let operations = parse_input(input).map_err(|e| {
        McpError::invalid_params(format!("Failed to parse kanban operation: {e}"), None)
    })?;

    // Execute each parsed operation and collect results. A single input
    // may parse to multiple operations (batch form); preserve ordering
    // and short-circuit on the first error.
    let mut results = Vec::with_capacity(operations.len());
    for op in &operations {
        let value = dispatch::execute_operation(ctx, op)
            .await
            .map_err(|e| classify_kanban_error(&op.op_string(), e))?;
        results.push(value);
    }

    // Collapse a single-op result to just that value so clients get the
    // same shape they would from a direct call; keep arrays for batches.
    let response = if results.len() == 1 {
        results.into_iter().next().expect("len == 1")
    } else {
        json!(results)
    };

    let text = serde_json::to_string_pretty(&response).unwrap_or_else(|_| response.to_string());

    Ok(CallToolResult::success(vec![Content::text(text)]))
}

/// Map a [`KanbanError`] to the appropriate [`McpError`] kind.
///
/// MCP clients distinguish three broad error classes: `invalid_params`
/// (the request body couldn't be interpreted), `invalid_request` (the
/// request was well-formed but references something that doesn't exist or
/// conflicts with server state), and `internal_error` (the server itself
/// failed — IO, lock, serialization). This classifier routes
/// `KanbanError` variants to the right class so clients can differentiate
/// user-fixable bad-input failures from server bugs.
///
/// `context` is prepended to the error message so the caller can tell
/// which operation in a batch triggered the failure (e.g.
/// `"move task: task not found: abc123"`).
fn classify_kanban_error(context: &str, err: KanbanError) -> McpError {
    let message = format!("{context}: {err}");
    match classify_kanban_error_kind(&err) {
        ErrorClass::InvalidParams => McpError::invalid_params(message, None),
        ErrorClass::InvalidRequest => McpError::invalid_request(message, None),
        ErrorClass::Internal => McpError::internal_error(message, None),
    }
}

/// MCP-facing error classes the kanban server maps `KanbanError` variants onto.
enum ErrorClass {
    /// Caller-fixable input problem (malformed body, missing/invalid field).
    InvalidParams,
    /// Well-formed request referencing state the server can't satisfy.
    InvalidRequest,
    /// Server-side failure the caller cannot fix.
    Internal,
}

/// Classify a [`KanbanError`] into an [`ErrorClass`] without building a message.
///
/// Split out from [`classify_kanban_error`] so the match stays focused on
/// pattern-to-class mapping; the caller handles message assembly.
fn classify_kanban_error_kind(err: &KanbanError) -> ErrorClass {
    match err {
        KanbanError::Parse { .. }
        | KanbanError::MissingField { .. }
        | KanbanError::InvalidValue { .. }
        | KanbanError::InvalidOperation { .. } => ErrorClass::InvalidParams,

        KanbanError::NotInitialized { .. }
        | KanbanError::AlreadyExists { .. }
        | KanbanError::TaskNotFound { .. }
        | KanbanError::ColumnNotFound { .. }
        | KanbanError::ActorNotFound { .. }
        | KanbanError::ProjectNotFound { .. }
        | KanbanError::TagNotFound { .. }
        | KanbanError::CommentNotFound { .. }
        | KanbanError::NotFound { .. }
        | KanbanError::ColumnNotEmpty { .. }
        | KanbanError::ProjectHasTasks { .. }
        | KanbanError::DuplicateId { .. }
        | KanbanError::DependencyCycle { .. } => ErrorClass::InvalidRequest,

        KanbanError::EntityError(inner) => classify_entity_error_kind(inner),

        KanbanError::LockBusy
        | KanbanError::LockTimeout { .. }
        | KanbanError::Io(_)
        | KanbanError::Json(_)
        | KanbanError::Yaml(_)
        | KanbanError::FieldsError(_)
        | KanbanError::ViewsError(_)
        | KanbanError::StoreError(_) => ErrorClass::Internal,
    }
}

/// Classify an [`EntityError`] for the entity-layer variants that can reach
/// the MCP surface without having been re-wrapped by
/// `KanbanError::from_entity_error` — notably `move task` and similar
/// direct-entity operations.
fn classify_entity_error_kind(err: &swissarmyhammer_entity::EntityError) -> ErrorClass {
    use swissarmyhammer_entity::EntityError;
    match err {
        EntityError::NotFound { .. }
        | EntityError::UnknownEntityType { .. }
        | EntityError::AttachmentNotFound { .. }
        | EntityError::AttachmentSourceNotFound { .. }
        | EntityError::AttachmentTooLarge { .. }
        | EntityError::ChangelogEntryNotFound { .. } => ErrorClass::InvalidRequest,

        EntityError::ValidationFailed { .. }
        | EntityError::StaleChange { .. }
        | EntityError::UnsupportedUndoOp { .. } => ErrorClass::InvalidParams,

        EntityError::InvalidFrontmatter { .. }
        | EntityError::Yaml { .. }
        | EntityError::YamlSerde(_)
        | EntityError::ComputeError { .. }
        | EntityError::PatchApply(_)
        | EntityError::TransactionPartialFailure { .. }
        | EntityError::RestoreFromTrashFailed { .. }
        | EntityError::Io(_)
        | EntityError::Store(_) => ErrorClass::Internal,
    }
}

/// Run the MCP kanban server over stdio until EOF.
///
/// Starts the `rmcp` stdio server with the kanban tool and blocks until
/// the MCP client disconnects or a fatal error occurs. Intended to be
/// called from the `serve` subcommand handler.
///
/// # Errors
///
/// Returns an error string if the server fails to start or encounters a
/// fatal error while serving.
pub async fn run_serve() -> Result<(), String> {
    use rmcp::serve_server;
    use rmcp::transport::io::stdio;

    let server = KanbanMcpServer::new();
    let running = serve_server(server, stdio())
        .await
        .map_err(|e| e.to_string())?;
    running.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::RawContent;
    use serde_json::json;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;
    use tempfile::TempDir;

    /// `get_info` must report `"kanban"` as the server implementation name
    /// so MCP clients can identify the binary. The version string must
    /// match the crate's `CARGO_PKG_VERSION` for build-diff visibility.
    #[test]
    fn get_info_reports_kanban_server_name() {
        let server = KanbanMcpServer::new();
        let info = server.get_info();

        assert_eq!(info.server_info.name, SERVER_NAME);
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    /// `get_info` must advertise the `tools` capability so MCP clients
    /// know to call `list_tools` during handshake. Without this, clients
    /// skip the tool listing step and the server appears to expose
    /// nothing.
    #[test]
    fn get_info_enables_tools_capability() {
        let server = KanbanMcpServer::new();
        let info = server.get_info();

        assert!(
            info.capabilities.tools.is_some(),
            "tools capability must be enabled for MCP clients to discover the kanban tool"
        );
    }

    /// `list_tools` must return exactly one tool named `"kanban"`. The
    /// single-tool contract is what lets this server stay minimal — the
    /// kanban tool internally dispatches to every noun/verb operation.
    ///
    /// Asserts against `build_list_tools_result` — the same function the
    /// `ServerHandler::list_tools` implementation calls — so any drift in
    /// the published shape is caught here.
    #[test]
    fn list_tools_returns_single_kanban_tool() {
        let result = build_list_tools_result();

        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools[0].name, TOOL_NAME);
        assert!(
            !result.tools[0].input_schema.is_empty(),
            "input_schema must be populated from kanban_operations()"
        );
        assert!(
            result.next_cursor.is_none(),
            "single-page result must have no cursor"
        );
    }

    /// `KanbanMcpServer` must remain `Clone` — the bound is required by
    /// rmcp's internals when the server is handed to `serve_server`. The
    /// `#[derive(Clone)]` already enforces this at compile time; this
    /// test documents the dependency so any future removal of the derive
    /// fails a test rather than silently breaking at the rmcp boundary.
    #[test]
    fn kanban_server_is_clone() {
        let server = KanbanMcpServer::new();
        let _clone = server.clone();
    }

    /// Build a fresh [`KanbanContext`] rooted at a new temp directory for
    /// call-dispatch tests. Returns the temp dir so the caller can keep
    /// it alive for the duration of the test.
    fn test_ctx() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().expect("create tempdir");
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        (temp, ctx)
    }

    /// Calling any tool name other than `"kanban"` must produce an
    /// `invalid_request` MCP error — the server only hosts a single
    /// tool, and unknown names are a client-side mistake, not a server
    /// bug.
    #[tokio::test]
    async fn call_tool_rejects_unknown_tool_name_as_invalid_request() {
        let (_temp, ctx) = test_ctx();
        let request = CallToolRequestParams::new("not-kanban");

        let err = dispatch_call_tool_request(&ctx, request)
            .await
            .expect_err("unknown tool name must be an error");

        assert_eq!(
            err.code,
            rmcp::model::ErrorCode::INVALID_REQUEST,
            "unknown tool name must map to invalid_request, got: {err:?}"
        );
        assert!(
            err.message.contains("Unknown tool"),
            "error message must identify the failure, got: {}",
            err.message
        );
    }

    /// A well-formed `init board` request must produce a success
    /// `CallToolResult` carrying the board JSON. Exercises the full
    /// pipeline: parse → dispatch → collapse single result → serialize.
    #[tokio::test]
    async fn call_tool_init_board_returns_success_response() {
        let (_temp, ctx) = test_ctx();
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("init board"));
        args.insert("name".to_string(), json!("Test Board"));
        let request = CallToolRequestParams::new(TOOL_NAME).with_arguments(args);

        let result = dispatch_call_tool_request(&ctx, request)
            .await
            .expect("init board must succeed in a fresh tempdir");

        assert_eq!(
            result.is_error,
            Some(false),
            "successful call_tool must not be flagged as error"
        );
        assert_eq!(
            result.content.len(),
            1,
            "single-op result must collapse to one content block"
        );

        let RawContent::Text(ref text) = result.content[0].raw else {
            panic!("expected text content, got: {:?}", result.content[0]);
        };
        let parsed: Value =
            serde_json::from_str(&text.text).expect("response text must be valid JSON");
        assert_eq!(
            parsed["name"], "Test Board",
            "response JSON must echo the created board name"
        );
    }

    /// Malformed input (an `op` string that doesn't map to any known
    /// verb/noun pair, and no other fields the inferrer can use) must
    /// map to `invalid_params` — the parser rejected it, and the caller
    /// can fix their request by supplying a valid `op`.
    #[tokio::test]
    async fn call_tool_malformed_input_returns_invalid_params() {
        let (_temp, ctx) = test_ctx();
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("bogus nonexistent"));
        let request = CallToolRequestParams::new(TOOL_NAME).with_arguments(args);

        let err = dispatch_call_tool_request(&ctx, request)
            .await
            .expect_err("unparseable op must fail to parse");

        assert_eq!(
            err.code,
            rmcp::model::ErrorCode::INVALID_PARAMS,
            "parse failure must map to invalid_params, got: {err:?}"
        );
        assert!(
            err.message.to_lowercase().contains("parse"),
            "error message must mention parse failure, got: {}",
            err.message
        );
    }

    /// A dispatch failure caused by a missing task id (well-formed
    /// request, but referencing state that doesn't exist) is a
    /// caller-addressable request error — not a server bug — and must
    /// surface as `invalid_request` via the error classifier.
    #[tokio::test]
    async fn call_tool_missing_task_returns_invalid_request() {
        let (_temp, ctx) = test_ctx();
        // Initialize the board so the failure comes from the missing
        // task, not from the board being un-initialized.
        let mut init_args = serde_json::Map::new();
        init_args.insert("op".to_string(), json!("init board"));
        init_args.insert("name".to_string(), json!("Classifier Test Board"));
        dispatch_call_tool_request(
            &ctx,
            CallToolRequestParams::new(TOOL_NAME).with_arguments(init_args),
        )
        .await
        .expect("init board must succeed");

        // Ask to move a task that doesn't exist — the engine returns
        // `KanbanError::TaskNotFound`, which must classify as
        // `invalid_request`.
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("move task"));
        args.insert("id".to_string(), json!("does-not-exist-01234567"));
        args.insert("column".to_string(), json!("doing"));
        let request = CallToolRequestParams::new(TOOL_NAME).with_arguments(args);

        let err = dispatch_call_tool_request(&ctx, request)
            .await
            .expect_err("move task on missing id must fail");

        assert_eq!(
            err.code,
            rmcp::model::ErrorCode::INVALID_REQUEST,
            "TaskNotFound must classify as invalid_request, got: {err:?}"
        );
        assert!(
            err.message.contains("move task"),
            "error message must prefix with the failing op_string, got: {}",
            err.message
        );
    }

    /// `KanbanMcpServer::kanban_dir` must resolve `.kanban` against the
    /// process CWD. Use `CurrentDirGuard` to serialize the CWD change
    /// and confirm dispatch round-trips a success response in that
    /// directory. Paths are compared after `canonicalize` because macOS
    /// tempdirs live under `/var/folders/...` which is a symlink to
    /// `/private/var/folders/...`.
    #[tokio::test]
    async fn kanban_server_call_tool_resolves_cwd_for_init_board() {
        let temp = TempDir::new().expect("create tempdir");
        let _guard = CurrentDirGuard::new(temp.path()).expect("enter tempdir");

        let kanban_dir = KanbanMcpServer::kanban_dir().expect("resolve .kanban dir");
        let expected = temp.path().join(".kanban");
        assert_eq!(
            kanban_dir.parent().and_then(|p| p.canonicalize().ok()),
            expected.parent().and_then(|p| p.canonicalize().ok()),
            "kanban_dir must resolve against the current CWD"
        );
        assert_eq!(
            kanban_dir.file_name(),
            Some(std::ffi::OsStr::new(".kanban")),
            "kanban_dir must end with `.kanban`"
        );

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), json!("init board"));
        args.insert("name".to_string(), json!("CWD Board"));
        let request = CallToolRequestParams::new(TOOL_NAME).with_arguments(args);

        let ctx = KanbanContext::new(kanban_dir);
        let result = dispatch_call_tool_request(&ctx, request)
            .await
            .expect("init board in guarded CWD must succeed");
        assert_eq!(result.is_error, Some(false));
    }

    /// The `classify_kanban_error` helper is the single place where
    /// `KanbanError` variants are routed to MCP error kinds. Direct unit
    /// coverage here catches regressions independently of the dispatch
    /// path — so reclassifying a variant can't silently break clients.
    #[test]
    fn classify_kanban_error_maps_caller_input_failures_to_invalid_params() {
        let cases = [
            KanbanError::parse("bad op"),
            KanbanError::missing_field("name"),
            KanbanError::invalid_value("order", "must be non-negative"),
            KanbanError::InvalidOperation {
                verb: "add".into(),
                noun: "nothing".into(),
            },
        ];
        for err in cases {
            let classified = classify_kanban_error("op", err);
            assert_eq!(
                classified.code,
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "bad-input variant must map to invalid_params, got: {classified:?}"
            );
        }
    }

    #[test]
    fn classify_kanban_error_maps_state_conflicts_to_invalid_request() {
        let cases = [
            KanbanError::NotInitialized {
                path: PathBuf::from("/tmp/.kanban"),
            },
            KanbanError::AlreadyExists {
                path: PathBuf::from("/tmp/.kanban"),
            },
            KanbanError::TaskNotFound { id: "x".into() },
            KanbanError::ColumnNotFound { id: "x".into() },
            KanbanError::not_found("attachment", "x"),
            KanbanError::duplicate_id("tag", "bug"),
            KanbanError::DependencyCycle {
                path: "a -> b -> a".into(),
            },
        ];
        for err in cases {
            let classified = classify_kanban_error("op", err);
            assert_eq!(
                classified.code,
                rmcp::model::ErrorCode::INVALID_REQUEST,
                "state-conflict variant must map to invalid_request, got: {classified:?}"
            );
        }
    }

    #[test]
    fn classify_kanban_error_maps_server_failures_to_internal_error() {
        let cases = [
            KanbanError::LockBusy,
            KanbanError::LockTimeout { elapsed_ms: 1000 },
            KanbanError::Io(std::io::Error::other("disk full")),
            KanbanError::FieldsError("broken registry".into()),
            KanbanError::ViewsError("broken view".into()),
            KanbanError::StoreError("db offline".into()),
        ];
        for err in cases {
            let classified = classify_kanban_error("op", err);
            assert_eq!(
                classified.code,
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "server-failure variant must map to internal_error, got: {classified:?}"
            );
        }
    }

    /// `EntityError::NotFound` sometimes leaks through as the generic
    /// `KanbanError::EntityError(..)` wrapper (for ops that don't go
    /// through `from_entity_error`, e.g. `move task`). The classifier
    /// must still recognize it as a caller-addressable
    /// `invalid_request`, not an internal server bug.
    #[test]
    fn classify_kanban_error_maps_entity_not_found_to_invalid_request() {
        let err = KanbanError::EntityError(swissarmyhammer_entity::EntityError::NotFound {
            entity_type: "task".into(),
            id: "abc".into(),
        });
        let classified = classify_kanban_error("move task", err);
        assert_eq!(
            classified.code,
            rmcp::model::ErrorCode::INVALID_REQUEST,
            "wrapped EntityError::NotFound must classify as invalid_request, got: {classified:?}"
        );
    }

    /// `EntityError::ValidationFailed` reports that the caller's field
    /// value didn't pass the schema — fixable with a corrected request,
    /// so the classifier routes it to `invalid_params`.
    #[test]
    fn classify_kanban_error_maps_entity_validation_to_invalid_params() {
        let err = KanbanError::EntityError(swissarmyhammer_entity::EntityError::ValidationFailed {
            field: "order".into(),
            message: "must be non-negative".into(),
        });
        let classified = classify_kanban_error("add column", err);
        assert_eq!(
            classified.code,
            rmcp::model::ErrorCode::INVALID_PARAMS,
            "wrapped EntityError::ValidationFailed must classify as invalid_params, got: {classified:?}"
        );
    }

    /// A genuinely-server-side `EntityError` (IO failure) must still
    /// classify as `internal_error` — the classifier must not blanket
    /// all `EntityError` variants as caller-addressable.
    #[test]
    fn classify_kanban_error_maps_entity_io_to_internal_error() {
        let err = KanbanError::EntityError(swissarmyhammer_entity::EntityError::Io(
            std::io::Error::other("disk unplugged"),
        ));
        let classified = classify_kanban_error("add task", err);
        assert_eq!(
            classified.code,
            rmcp::model::ErrorCode::INTERNAL_ERROR,
            "wrapped EntityError::Io must classify as internal_error, got: {classified:?}"
        );
    }

    /// The classifier must prepend the operation context to the error
    /// message so batched-op failures identify which op broke — mirrors
    /// the pre-refactor format `{op}: {err}`.
    #[test]
    fn classify_kanban_error_prepends_context_to_message() {
        let err = KanbanError::TaskNotFound { id: "abc".into() };
        let classified = classify_kanban_error("move task", err);
        assert!(
            classified.message.starts_with("move task: "),
            "message must be prefixed with op context, got: {}",
            classified.message
        );
        assert!(
            classified.message.contains("abc"),
            "message must retain underlying error text, got: {}",
            classified.message
        );
    }
}
