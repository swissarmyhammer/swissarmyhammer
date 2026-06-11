---
assignees:
- claude-code
depends_on:
- 01KTVPZ1VE36FVG8CMQ49X8RMK
position_column: todo
position_ordinal: 9f80
project: mirdan-install
title: 'kanban-app: "Expose this board to your agent" command + UI'
---
## What
Per-board GUI action in the kanban desktop app that registers the kanban MCP server into every mirdan-detected agent's **project-scope** config, rooted at the board root (the directory containing `.kanban/`). Settled design: project scope only; entry is `McpServerEntry { command: <absolute path to bundled kanban CLI>, args: ["serve"], env: {} }` — `kanban serve` resolves the board from process CWD (`apps/kanban-cli/src/commands/serve.rs:63-67`), and project-scope registration means the agent's CWD is the board root, so no `--board` flag (do not add one).

Backend (`apps/kanban-app`, which already depends on mirdan — `apps/kanban-app/Cargo.toml:29`):
- New `#[tauri::command] expose_board_to_agents(board_path: ...)` — this is an OS-level file operation, NOT board-state mutation, so a plain Tauri command is correct per the `apps/kanban-app/src/commands.rs:1-22` header (do not route through `dispatch_command`). Follow the existing pattern of an extracted inner function (`commands.rs:3009` comment) so logic is testable without Tauri: `fn expose_board_to_agents_inner(board_root: &Path, cli_path: &Path) -> Vec<AgentExposeResult>`.
- CLI path resolution: reuse `resolve_bundled_cli(current_exe)` (`apps/kanban-app/src/cli_install.rs:89` — the CLI is already bundled as a Tauri sidecar, `apps/kanban-app/tauri.conf.json` `externalBin: ["binaries/kanban"]`; no bundling work needed). When it returns `None` (dev `cargo run` with no staged sidecar), fall back to the dev sidecar staged by `scripts/before-dev.sh` next to the exe or return a structured error surfaced in the UI — pick whichever the before-dev script makes feasible and unit-test it.
- Registration: call `mirdan::install::register_mcp_server_at(board_root, "kanban", &entry, InitScope::Project, &reporter)` (made public by the prerequisite task) and map the returned `Vec<InitResult>` to per-agent `{agent, ok, message}` results returned to the frontend. The board root is passed explicitly per window (same root as `start_board_mcp_server`, `apps/kanban-app/src/state.rs:1275`, and `deploy_workspace_tools`, `state.rs:1220`). NOTHING on this path may call `std::env::current_dir()` — the bundled GUI launches with CWD `/` (read-only).
- Register the command in `tauri::generate_handler![...]` (`apps/kanban-app/src/main.rs:57`).

Frontend: a board-level action ("Expose this board to your agent") in the board menu (`apps/kanban-app/src/menu.rs` grouped-submenu pattern) or board toolbar, invoking the Tauri command with the window's board path and showing per-agent success/failure from the returned results.

- [ ] `expose_board_to_agents_inner(board_root, cli_path)` + `#[tauri::command]` wrapper, registered in `main.rs`
- [ ] Bundled-CLI path resolution with dev-mode fallback (unit-tested)
- [ ] Frontend action + per-agent result display
- [ ] Integration + unit tests (see Tests)

## Acceptance Criteria
- [ ] Invoking the command with a temp board root and a fake agents config writes the expected project-scope files under that root (e.g. `.mcp.json`) containing the absolute CLI path and `args: ["serve"]`
- [ ] Works with process CWD set elsewhere (no `current_dir()` reads on this path)
- [ ] Per-agent results (success/failure per detected agent) are returned to and rendered by the frontend
- [ ] No `--board` flag added to `kanban serve`; CLI entry shape in `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` untouched

## Tests
- [ ] Rust integration test (e.g. `apps/kanban-app/tests/` or `#[cfg(test)]` beside the inner fn, matching existing app test layout): temp board root + fake agents YAML via the `MIRDAN_AGENTS_CONFIG` env override (`crates/mirdan/src/agents.rs:140`), call `expose_board_to_agents_inner`, assert config files appear under the temp root with the absolute binary path; use CWD-isolation per the project's CurrentDirGuard/serial_test convention
- [ ] Unit test for the CLI path resolver dev fallback (bundled present / absent)
- [ ] `cargo test -p kanban-app` passes with 0 failures

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.