//! Per-client served-set composition at the serve boundary.
//!
//! The SAH MCP server composes the tool set it advertises **per connecting
//! client**, driven by the MCP `initialize` handshake's client `Implementation`
//! name and each tool's `category()`. These tests drive `tools/list` through the
//! real rmcp serve/handshake (an in-process HTTP server + a real rmcp client)
//! under different client identities, asserting the advertised set rather than
//! calling the internal filter directly:
//!
//! - A **Claude** client is advertised `Shared` + `Replacement` tools — `shell`
//!   is present, but `Agent`-category tools (web/skill/agent/files) are not.
//! - A **llama** client is advertised `Shared` only — no `shell`, no `Agent`
//!   tools.
//! - An **unknown** client gets the conservative default: `Shared` only.
//!
//! The reference pattern is `rmcp_stdio_working.rs`: an in-process HTTP MCP
//! server bound in an isolated tempdir, driven by a real rmcp client.

use std::collections::HashSet;

use serial_test::serial;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client_named,
    unified_server::{start_mcp_server_with_options, McpServerHandle, McpServerMode},
};

use super::mirdan_test_support::{write_claude_agents_config, MirdanConfigGuard};

/// `Replacement`-category tool: served only to Claude.
const SHELL_TOOL: &str = "shell";

/// `Agent`-category tools: never advertised by SAH to any host.
const AGENT_TOOLS: &[&str] = &["web", "skill", "agent", "files"];

/// `Shared`-category tools: advertised to every host. `kanban` and
/// `code_context` are stable domain capabilities that exercise the `Shared`
/// path.
const SHARED_TOOLS: &[&str] = &["kanban", "code_context"];

/// Start an in-process HTTP MCP server in an isolated tempdir.
///
/// The tempdir keeps `initialize_code_context` from walking the host monorepo
/// (see `rmcp_stdio_working.rs`). The returned `TempDir` must be kept alive for
/// the duration of the test.
async fn start_isolated_server() -> (McpServerHandle, tempfile::TempDir) {
    let temp = tempfile::TempDir::new().expect("Failed to create temp dir");
    let server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("Failed to start in-process MCP server");
    (server, temp)
}

/// Drive `tools/list` over a real rmcp handshake under `client_name`, returning
/// the advertised tool names.
async fn advertised_tools(server: &McpServerHandle, client_name: &str) -> HashSet<String> {
    let client = create_test_client_named(server.url(), client_name).await;
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("tools/list must succeed");
    let names: HashSet<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    client.cancel().await.expect("Failed to cancel client");
    names
}

fn assert_none_present(names: &HashSet<String>, forbidden: &[&str], context: &str) {
    for &tool in forbidden {
        assert!(
            !names.contains(tool),
            "{context}: tool `{tool}` must NOT be advertised, but it was. \
             Advertised: {names:?}"
        );
    }
}

fn assert_all_present(names: &HashSet<String>, required: &[&str], context: &str) {
    for &tool in required {
        assert!(
            names.contains(tool),
            "{context}: tool `{tool}` must be advertised, but it was not. \
             Advertised: {names:?}"
        );
    }
}

/// A Claude client (`"claude-code"`) is advertised `Shared` + `Replacement`:
/// `shell` is present, every `Agent` tool is absent.
///
/// Connecting a Claude client triggers the serve-time native-deny path, which
/// reads the process-global `MIRDAN_AGENTS_CONFIG`. This test therefore joins
/// the shared `#[serial(mirdan_env)]` group and redirects that env var into its
/// own isolated tempdir (via [`MirdanConfigGuard`]) so the handshake's deny can
/// never leak into the real environment or another test's tempdir, regardless
/// of run ordering.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(mirdan_env)]
async fn claude_client_gets_shared_plus_shell_not_agent_tools() {
    let (mut server, temp) = start_isolated_server().await;
    let _guard = MirdanConfigGuard::set(&write_claude_agents_config(temp.path()));

    let names = advertised_tools(&server, "claude-code").await;

    assert_all_present(&names, SHARED_TOOLS, "claude");
    assert!(
        names.contains(SHELL_TOOL),
        "claude: the Replacement-category `shell` tool must be advertised. \
         Advertised: {names:?}"
    );
    assert_none_present(&names, AGENT_TOOLS, "claude");

    server.shutdown().await.expect("Failed to shutdown server");
}

/// A llama client (`"llama_agent_notifying_client"`) is advertised `Shared`
/// only: no `shell` (it mounts its own), no `Agent` tools.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn llama_client_gets_shared_only() {
    let (mut server, _temp) = start_isolated_server().await;

    let names = advertised_tools(&server, "llama_agent_notifying_client").await;

    assert_all_present(&names, SHARED_TOOLS, "llama");
    assert!(
        !names.contains(SHELL_TOOL),
        "llama: the Replacement-category `shell` tool must NOT be advertised \
         (llama mounts its own). Advertised: {names:?}"
    );
    assert_none_present(&names, AGENT_TOOLS, "llama");

    server.shutdown().await.expect("Failed to shutdown server");
}

/// An unknown client gets the conservative default — `Shared` only: no `shell`,
/// no `Agent` tools.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_client_gets_shared_only() {
    let (mut server, _temp) = start_isolated_server().await;

    let names = advertised_tools(&server, "some-unrecognized-mcp-client").await;

    assert_all_present(&names, SHARED_TOOLS, "unknown");
    assert!(
        !names.contains(SHELL_TOOL),
        "unknown host: the Replacement-category `shell` tool must NOT be \
         advertised. Advertised: {names:?}"
    );
    assert_none_present(&names, AGENT_TOOLS, "unknown");

    server.shutdown().await.expect("Failed to shutdown server");
}
