//! Serve-time native-tool deny, gated on the connecting Claude client.
//!
//! When a **Claude** client connects to the SAH MCP server, the serve path
//! writes a deny for the native host tool(s) the served `Replacement` tools
//! supersede (today `shell` replaces `"Bash"`) into Claude's local settings, so
//! the served tool truly replaces the native rather than competing with it. A
//! llama or unknown client triggers no deny.
//!
//! These tests drive the real rmcp `initialize` handshake (an in-process HTTP
//! server + a real rmcp client, mirroring `per_client_tool_composition.rs`)
//! under different client identities and assert the deny by reading the settings
//! file the serve path writes — not by spying the mirdan call. mirdan's agent
//! detection and settings paths are redirected into a tempdir via the
//! `MIRDAN_AGENTS_CONFIG` env var, so the test writes nowhere near a real
//! `.claude/`.
//!
//! The env var is process-global, so these tests join the shared
//! `#[serial(mirdan_env)]` group (see [`mirdan_test_support`]) to avoid racing
//! any other test — in this binary — that reads or writes mirdan's agents
//! config, including the Claude handshake in `per_client_tool_composition.rs`.

use std::path::Path;

use serial_test::serial;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client_named,
    unified_server::{start_mcp_server_with_options, McpServerHandle, McpServerMode},
};

use super::mirdan_test_support::{write_claude_agents_config, MirdanConfigGuard};

/// Start an in-process HTTP MCP server in an isolated tempdir.
///
/// The tempdir keeps `initialize_code_context` from walking the host monorepo
/// (mirrors `per_client_tool_composition.rs`).
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

/// Drive the `initialize` handshake under `client_name` and immediately cancel.
///
/// The serve-time deny runs inside `initialize`, so once `.serve()` returns the
/// deny (if any) has already been written.
async fn handshake_as(server: &McpServerHandle, client_name: &str) {
    let client = create_test_client_named(server.url(), client_name).await;
    client.cancel().await.expect("Failed to cancel client");
}

/// Read the `permissions.deny` array from a settings file, or empty if absent.
fn deny_list(settings_path: &Path) -> Vec<String> {
    if !settings_path.exists() {
        return Vec::new();
    }
    let text = std::fs::read_to_string(settings_path).expect("read settings file");
    let json: serde_json::Value = serde_json::from_str(&text).expect("parse settings json");
    json["permissions"]["deny"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// A Claude client connecting at serve time has `Bash` denied in Claude's
/// local settings (`.claude/settings.local.json`), derived from the served
/// `Replacement{native:"Bash"}` shell tool.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(mirdan_env)]
async fn claude_client_triggers_bash_deny() {
    let (mut server, temp) = start_isolated_server().await;
    let _guard = MirdanConfigGuard::set(&write_claude_agents_config(temp.path()));

    handshake_as(&server, "claude-code").await;

    let local_settings = temp.path().join(".claude/settings.local.json");
    let denies = deny_list(&local_settings);
    assert!(
        denies.iter().any(|t| t == "Bash"),
        "Claude connect must deny Bash in settings.local.json; deny list: {denies:?}"
    );
    // Local scope must not touch the committed project settings.json.
    assert!(
        !temp.path().join(".claude/settings.json").exists(),
        "Local-scope deny must not write the committed settings.json"
    );

    server.shutdown().await.expect("Failed to shutdown server");
}

/// A llama client connecting at serve time triggers no deny — it mounts its own
/// shell and has no native `Bash` to suppress.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(mirdan_env)]
async fn llama_client_triggers_no_deny() {
    let (mut server, temp) = start_isolated_server().await;
    let _guard = MirdanConfigGuard::set(&write_claude_agents_config(temp.path()));

    handshake_as(&server, "llama_agent_notifying_client").await;

    let local_settings = temp.path().join(".claude/settings.local.json");
    assert!(
        deny_list(&local_settings).is_empty(),
        "llama connect must not write any deny; found: {:?}",
        deny_list(&local_settings)
    );

    server.shutdown().await.expect("Failed to shutdown server");
}

/// An unknown client connecting at serve time triggers no deny — only Claude
/// hosts have their native tools suppressed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(mirdan_env)]
async fn unknown_client_triggers_no_deny() {
    let (mut server, temp) = start_isolated_server().await;
    let _guard = MirdanConfigGuard::set(&write_claude_agents_config(temp.path()));

    handshake_as(&server, "some-unrecognized-mcp-client").await;

    let local_settings = temp.path().join(".claude/settings.local.json");
    assert!(
        deny_list(&local_settings).is_empty(),
        "unknown client must not write any deny; found: {:?}",
        deny_list(&local_settings)
    );

    server.shutdown().await.expect("Failed to shutdown server");
}
