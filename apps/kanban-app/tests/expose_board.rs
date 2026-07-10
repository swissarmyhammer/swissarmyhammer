//! Integration test for the "Expose this board to your agent" registration core.
//!
//! The `expose` module is part of the `kanban-app` binary crate, which has no
//! library target, so this file compiles the module source into the test binary
//! via `#[path]` — the same independent-compilation pattern `tests/cli_install.rs`
//! uses for binary-crate modules.
//!
//! The test drives [`expose::expose_board_to_agents_inner`] against a temp board
//! root and a hermetic fake-agent config (via `MIRDAN_AGENTS_CONFIG`), with the
//! process CWD pinned to a *different* temp dir, proving the registration is
//! rooted purely at the explicit `board_root` and never reads `current_dir()`.

#[path = "../src/expose.rs"]
#[allow(dead_code)]
mod expose;

use std::path::PathBuf;

use expose::expose_board_to_agents_inner;
use mirdan::test_support::{write_fake_agents_config, MirdanConfigGuard};
use serial_test::serial;
use swissarmyhammer_common::test_utils::CurrentDirGuard;

/// Exposing a board writes the detected agent's project-scope MCP config
/// (`.mcp.json`) UNDER the explicit board root — carrying the absolute CLI path
/// and `args: ["serve"]` — even when the process CWD points somewhere else, and
/// returns a per-agent success result.
#[test]
#[serial(cwd)]
fn expose_writes_project_scope_mcp_under_board_root_with_absolute_cli() {
    // The process CWD and the registration root are deliberately distinct temp
    // dirs: any read of `current_dir()` would write to the wrong place and the
    // no-CWD assertion below would fail.
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&cwd).unwrap();

    // A fake single-agent config whose detect dir is the CWD itself (so
    // detection always fires) and whose project MCP config is a relative
    // `.mcp.json` (Claude Code shape).
    let config_path = write_fake_agents_config(&cwd);
    let _mirdan = MirdanConfigGuard::set(&config_path);

    // The board root the user is exposing — distinct from the CWD.
    let board = tempfile::tempdir().unwrap();
    let board_root = board.path().canonicalize().unwrap();

    // An absolute, bundled-style CLI path (as `resolve_bundled_cli` would yield).
    let cli_path = PathBuf::from("/Applications/Kanban.app/Contents/MacOS/kanban");

    let results = expose_board_to_agents_inner(&board_root, &cli_path);

    // Per-agent results are returned, and the detected agent registered cleanly.
    assert!(
        !results.is_empty(),
        "expose must return a per-agent result for the detected agent"
    );
    assert!(
        results.iter().all(|r| r.ok),
        "the detected agent must register successfully: {results:?}"
    );

    // The project-scope `.mcp.json` lands under the BOARD ROOT, not the CWD.
    let mcp = board_root.join(".mcp.json");
    assert!(
        mcp.is_file(),
        ".mcp.json must be written under the board root: {}",
        mcp.display()
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp).unwrap()).unwrap();
    assert_eq!(
        json["mcpServers"]["kanban"]["command"],
        cli_path.display().to_string(),
        "the registered command must be the absolute bundled CLI path"
    );
    assert_eq!(
        json["mcpServers"]["kanban"]["args"][0], "serve",
        "the registered args must be exactly [\"serve\"] — no --board flag"
    );

    // Nothing was written relative to the process CWD.
    assert!(
        !cwd.join(".mcp.json").exists(),
        "expose must not write under the process CWD"
    );
}

/// When NO agent is detected, exposing the board registers nothing and returns
/// an EMPTY result list — so the frontend renders an informational "no agents"
/// message rather than a misleading green "applied to 0 agent(s)" success.
#[test]
#[serial(cwd)]
fn expose_returns_empty_when_no_agents_detected() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().canonicalize().unwrap();
    let _cwd = CurrentDirGuard::new(&cwd).unwrap();

    // An agents config with no agents at all — detection yields nothing.
    let config_path = cwd.join("agents.yaml");
    std::fs::write(&config_path, "agents: []\n").unwrap();
    let _mirdan = MirdanConfigGuard::set(&config_path);

    let board = tempfile::tempdir().unwrap();
    let board_root = board.path().canonicalize().unwrap();
    let cli_path = PathBuf::from("/Applications/Kanban.app/Contents/MacOS/kanban");

    let results = expose_board_to_agents_inner(&board_root, &cli_path);

    assert!(
        results.is_empty(),
        "no detected agent must yield no results (got {results:?}) so the UI \
         shows a 'no agents' message, not a 0-agent success"
    );
    assert!(
        !board_root.join(".mcp.json").exists(),
        "nothing must be written when no agent is detected"
    );
}
