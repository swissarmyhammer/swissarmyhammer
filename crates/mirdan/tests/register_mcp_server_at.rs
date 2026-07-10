//! External-crate coverage for the root-explicit MCP registration API.
//!
//! These tests live outside `mirdan` (compiled as their own crate) to enforce
//! the acceptance criterion that `mirdan::install::register_mcp_server_at` and
//! `unregister_mcp_server_at` are reachable from another crate — the kanban-app
//! GUI ("Expose this board to your agent") calls them from a process whose CWD
//! is `/` and read-only, so registration must target a caller-supplied root and
//! never read `current_dir()`.

use mirdan::mcp_config::McpServerEntry;
use mirdan::test_support::{write_fake_agents_config, MirdanConfigGuard};
use swissarmyhammer_common::lifecycle::InitScope;
use swissarmyhammer_common::reporter::NullReporter;

/// `register_mcp_server_at` is part of the public API and writes the entry into
/// the agent's MCP config resolved against the explicit root, not the CWD.
#[test]
fn register_mcp_server_at_is_public_and_writes_under_root() {
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = write_fake_agents_config(config_dir.path());
    let _mirdan = MirdanConfigGuard::set(&config_path);

    // The registration root is distinct from the config/detect dir and from the
    // process CWD — nothing here changes `current_dir()`.
    let root = tempfile::tempdir().unwrap();
    let entry = McpServerEntry {
        command: "/opt/sah/bin/sah".to_string(),
        args: vec!["serve".to_string()],
        env: std::collections::BTreeMap::new(),
    };

    let reporter = NullReporter;
    let results = mirdan::install::register_mcp_server_at(
        root.path(),
        "sah",
        &entry,
        InitScope::Project,
        &reporter,
    );
    assert!(
        results
            .iter()
            .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error),
        "register_mcp_server_at must not error: {results:?}"
    );

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.path().join(".mcp.json")).unwrap())
            .unwrap();
    assert_eq!(json["mcpServers"]["sah"]["command"], entry.command);
    assert_eq!(json["mcpServers"]["sah"]["args"][0], entry.args[0]);

    // unregister_mcp_server_at is likewise public and removes what it wrote.
    let results = mirdan::install::unregister_mcp_server_at(
        root.path(),
        "sah",
        InitScope::Project,
        &reporter,
    );
    assert!(results
        .iter()
        .all(|r| r.status != swissarmyhammer_common::lifecycle::InitStatus::Error));
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.path().join(".mcp.json")).unwrap())
            .unwrap();
    assert!(json["mcpServers"]["sah"].is_null());
}
