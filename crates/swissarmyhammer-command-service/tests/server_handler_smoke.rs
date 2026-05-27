//! `rmcp::ServerHandler` smoke tests for [`CommandService`].
//!
//! These assertions pin the contract that `CommandService` advertises
//! exactly one tool named `command` whose `_meta` carries the full
//! operations discovery tree with all six verbs.

use rmcp::model::{NumberOrString, PaginatedRequestParams};
use rmcp::service::{serve_directly, Peer, RequestContext, RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServerHandler};
use std::future::Future;
use swissarmyhammer_command_service::CommandService;

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

/// Mints a `Peer<RoleServer>` by briefly serving a placeholder handler.
fn mint_peer() -> Peer<RoleServer> {
    struct PeerProbe;
    impl ServerHandler for PeerProbe {}

    let running = serve_directly(PeerProbe, ClosedTransport, None);
    running.peer().clone()
}

/// Helper: build a `RequestContext` against an inert peer.
fn request_context() -> RequestContext<RoleServer> {
    RequestContext::new(NumberOrString::Number(0), mint_peer())
}

#[tokio::test]
async fn list_tools_returns_exactly_one_command_tool() {
    let service = CommandService::new();
    let result = service
        .list_tools(None::<PaginatedRequestParams>, request_context())
        .await
        .expect("list_tools should succeed");

    assert_eq!(
        result.tools.len(),
        1,
        "CommandService should advertise exactly one tool"
    );
    assert_eq!(
        result.tools[0].name, "command",
        "the single tool must be named `command`"
    );
}

#[tokio::test]
async fn command_tool_meta_carries_all_six_verbs() {
    let service = CommandService::new();
    let result = service
        .list_tools(None::<PaginatedRequestParams>, request_context())
        .await
        .expect("list_tools should succeed");

    let tool = &result.tools[0];
    let meta = tool
        .meta
        .as_ref()
        .expect("`command` tool must carry `_meta`");
    let ops_meta = meta
        .0
        .get("io.swissarmyhammer/operations")
        .expect("_meta must carry io.swissarmyhammer/operations");

    let command_noun = ops_meta
        .as_object()
        .expect("operations meta should be an object")
        .get("command")
        .expect("operations meta must carry the `command` noun")
        .as_object()
        .expect("`command` noun must be an object");

    for verb in [
        "register",
        "unregister",
        "execute",
        "available",
        "list",
        "schema",
    ] {
        assert!(
            command_noun.contains_key(verb),
            "meta.command.{verb:?} missing — got {:?}",
            command_noun.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        command_noun.len(),
        6,
        "expected exactly six verbs under meta.command"
    );
}

#[tokio::test]
async fn command_tool_input_schema_is_flat_op_enum() {
    let service = CommandService::new();
    let result = service
        .list_tools(None::<PaginatedRequestParams>, request_context())
        .await
        .expect("list_tools should succeed");

    let schema = &result.tools[0].input_schema;
    let op_enum = schema["properties"]["op"]["enum"]
        .as_array()
        .expect("inputSchema.properties.op.enum should be an array");
    let ops: Vec<&str> = op_enum.iter().filter_map(|v| v.as_str()).collect();
    for expected in [
        "register command",
        "unregister command",
        "execute command",
        "available command",
        "list command",
        "schema command",
    ] {
        assert!(
            ops.contains(&expected),
            "op enum missing {expected:?}: {ops:?}"
        );
    }
}
