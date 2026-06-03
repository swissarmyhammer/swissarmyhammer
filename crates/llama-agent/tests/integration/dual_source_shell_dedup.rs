//! Real-path test: a llama-agent fed BOTH its in-process Agent-tools mount AND
//! an external SAH project server sees `shell` (and every Agent tool) exactly
//! once.
//!
//! This is the realistic dual-source topology the agent assembles in
//! `AcpServer::new_session`: the session's MCP clients are `[mount_client,
//! ...external_clients]`, and `Session.available_tools` is the *concatenation*
//! of every client's `list_tools_with_schemas()` (see
//! `acp/server.rs::new_session` — "No cross-client name-collision dedup"). That
//! aggregation has no name-collision dedup, so "shell appears exactly once"
//! holds only because the per-client composition (card #2) withholds `shell`
//! (and the other Agent/Replacement tools) from a **llama** client connecting to
//! the SAH server. This test proves that invariant against the production
//! pieces — if the SAH server ever stopped host-filtering, or the mount started
//! double-mounting, the duplicate would surface here.
//!
//! The two sources, exactly as production builds them:
//!   (a) The in-process Agent-tools mount: `InProcessMount` over the real
//!       `McpServer::create_agent_tools_server()` (`compose_per_client = false`,
//!       serves files/web/skill/agent/shell verbatim).
//!   (b) An external SAH project server: the real per-client-composing
//!       `McpServer` served over HTTP, connected by a **llama** client (the
//!       `UnifiedMCPClient` HTTP handler advertises
//!       `llama_agent_notifying_client`, so the SAH server composes it
//!       **Shared only** — kanban/git/code_context/ralph/question, no
//!       shell/agent tools).
//!
//! The aggregation below mirrors `new_session`'s loop verbatim: collect every
//! client's `list_tools_with_schemas()` into one flat `Vec`, then assert the
//! served set.

use std::collections::HashMap;

use llama_agent::{AgentToolsMount, InProcessMount, MCPClient, UnifiedMCPClient};
use swissarmyhammer_prompts::PromptLibrary;
use swissarmyhammer_tools::mcp::unified_server::{
    start_mcp_server_with_options, McpServerHandle, McpServerMode,
};
use swissarmyhammer_tools::McpServer;

/// Tools the agent gets ONLY from the in-process mount (the SAH server withholds
/// every one of these from a llama client). Each must appear exactly once.
const MOUNT_ONLY_TOOLS: &[&str] = &["shell", "files", "web", "skill", "agent"];

/// `Shared`-category tools the external SAH server advertises to every host,
/// including llama. Stable anchors proving the external source contributed.
const SHARED_ANCHOR_TOOLS: &[&str] = &["kanban", "code_context"];

/// Start an in-process HTTP SAH MCP server in an isolated tempdir.
///
/// `compose_per_client` is `true` for this primary serve path, so the server
/// host-filters the advertised set by the connecting client's identity — the
/// behavior under test. The tempdir keeps `initialize_code_context` from walking
/// the host monorepo; it must outlive the returned handle.
async fn start_isolated_sah_server() -> (McpServerHandle, tempfile::TempDir) {
    let temp = tempfile::TempDir::new().expect("create temp dir for SAH server");
    let server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("start in-process SAH MCP server");
    (server, temp)
}

/// Aggregate every client's `list_tools_with_schemas()` into one flat list,
/// exactly as `AcpServer::new_session` builds `Session.available_tools`.
async fn aggregate_tool_names(clients: &[&dyn MCPClient]) -> Vec<String> {
    let mut names = Vec::new();
    for client in clients {
        let tools = client
            .list_tools_with_schemas()
            .await
            .expect("list_tools_with_schemas over MCP client");
        names.extend(tools.into_iter().map(|t| t.name));
    }
    names
}

/// With BOTH the in-process Agent-tools mount and an external (llama-facing) SAH
/// server, the aggregated tool set contains `shell` and every other Agent tool
/// exactly once, the external Shared tools, and NO duplicate tool names.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn llama_dual_source_aggregation_has_shell_exactly_once() {
    // Source (a): the real in-process Agent-tools mount.
    let sah = McpServer::new(PromptLibrary::default())
        .await
        .expect("build SAH McpServer");
    let mount = InProcessMount::new(sah.create_agent_tools_server());
    let mount_client = mount
        .connect()
        .await
        .expect("connect to in-process agent-tools mount");

    // Source (b): the external per-client-composing SAH server, connected by a
    // llama client. `UnifiedMCPClient`'s HTTP handler advertises
    // `llama_agent_notifying_client`, so the server composes it Shared-only.
    let (mut server, _temp) = start_isolated_sah_server().await;
    let external_client = UnifiedMCPClient::with_streamable_http(server.url(), Some(30))
        .await
        .expect("connect llama client to external SAH server");

    // Aggregate exactly as the session does: mount first, then external.
    let names =
        aggregate_tool_names(&[mount_client.as_ref(), &external_client as &dyn MCPClient]).await;

    // Count occurrences once; every assertion reads from this map.
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for name in &names {
        *counts.entry(name.as_str()).or_default() += 1;
    }

    // The crux: every mount-only Agent tool — `shell` foremost — appears EXACTLY
    // once. A count of 2 would mean the external SAH server also served it (i.e.
    // the per-client composition stopped host-filtering) and the dedup-free
    // aggregation double-registered it.
    for &tool in MOUNT_ONLY_TOOLS {
        assert_eq!(
            counts.get(tool).copied().unwrap_or(0),
            1,
            "mount-only Agent tool `{tool}` must appear exactly once across the \
             dual-source aggregation (mount + llama-facing SAH server). \
             A count != 1 means the SAH server stopped withholding it from llama \
             or the mount double-registered. Aggregated set: {names:?}"
        );
    }

    // The external Shared tools must be present — proving the external source
    // actually contributed and the test isn't trivially green from the mount
    // alone.
    for &tool in SHARED_ANCHOR_TOOLS {
        assert!(
            counts.contains_key(tool),
            "external SAH server must contribute Shared tool `{tool}` to the \
             aggregation. Aggregated set: {names:?}"
        );
    }

    // The load-bearing global invariant: NO tool name is duplicated anywhere in
    // the aggregated set, for the real SAH topology.
    let duplicates: Vec<(&str, usize)> = counts
        .iter()
        .filter(|&(_, &n)| n > 1)
        .map(|(&name, &n)| (name, n))
        .collect();
    assert!(
        duplicates.is_empty(),
        "no tool name may appear more than once in the dual-source aggregation, \
         but these did: {duplicates:?}. The cross-client aggregation has no name \
         dedup, so exactly-once relies on the per-client SAH composition \
         withholding mount-owned tools from llama. Aggregated set: {names:?}"
    );

    server
        .shutdown()
        .await
        .expect("shutdown external SAH server");
}
