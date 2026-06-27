---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3fnybrwc58j1gmfbmq99xf
  text: |-
    Picked up. Research complete. Design decisions:

    1. Backend split for standalone testability: `expose_board_to_agents_inner(board_root, cli_path) -> Vec<AgentExposeResult>` + `AgentExposeResult` + a private collecting `InitReporter` live in a NEW self-contained module `apps/kanban-app/src/expose.rs` depending ONLY on external crates (mirdan, swissarmyhammer-common, serde, std) — NO `crate::` refs — so the integration test can `#[path = "../src/expose.rs"] mod expose;` it (the kanban-app binary has no lib target; this is the same pattern tests/cli_install.rs uses).

    2. Per-agent results via collecting reporter, NOT the returned Vec<InitResult>. Reality check on the prerequisite: `register_mcp_server_at` returns a single SUMMARY InitResult ("Registered applied to N agent(s)") and emits the per-agent info as reporter events (Action per changed agent, Warning per error). So the inner fn passes a collecting reporter and maps Action->{ok:true}, Warning/Error->{ok:false}; falls back to the summary when no agent changed. DEVIATION from the card's literal `{agent, ok, message}` shape: `AgentExposeResult` is `{ok, message}` only — the per-agent identity is carried IN the message (each event names its agent: "kanban MCP server for <Agent>"). Parsing a separate `agent` field out of cross-crate message strings would be brittle; the message already identifies the agent, satisfying "per-agent success/failure rendered".

    3. CLI path: reuse `cli_install::resolve_bundled_cli(current_exe)` in the Tauri wrapper (commands.rs). The dev sidecar that before-dev.sh / `cargo build -p kanban-cli` stages lands at target/<profile>/kanban — a SIBLING of the kanban-app exe — so resolve_bundled_cli already finds it in dev; no separate dev path. None -> structured error string guiding the dev to run before-dev.sh. resolve_bundled_cli present/absent is already unit-tested in tests/cli_install.rs.

    4. Entry: McpServerEntry { command: <abs cli path>, args: ["serve"], env: {} }, scope Project, server name "kanban". No --board flag.

    5. UI: plain `#[tauri::command]` (OS-level op, not dispatch_command). Affordance = a board-toolbar button in BoardSelector next to the existing tear-off (gated `showTearOff && selectedPath`, same Pressable+Tooltip pattern), invoking `expose_board_to_agents` with the board path and rendering per-agent results as sonner toasts. (The native menu is built purely from the command-service registry, which routes through dispatch_command — wrong path for a plain Tauri command — so a React affordance is the correct fit.)

    Following /tdd.
  timestamp: 2026-06-27T02:48:59.000500+00:00
- actor: claude-code
  id: 01kw3gq2h146rbxnnz1wp6g3xr
  text: |-
    Implementation landed (TDD throughout). Files changed:

    Backend (Rust):
    - apps/kanban-app/src/expose.rs (NEW): `AgentExposeResult {ok, message}`, a private collecting `InitReporter`, and `expose_board_to_agents_inner(board_root, cli_path) -> Vec<AgentExposeResult>` calling `mirdan::install::register_mcp_server_at(board_root, "kanban", &McpServerEntry{command:cli_path, args:["serve"], env:{}}, InitScope::Project, &collector)`. Standalone-compilable (no crate:: refs).
    - apps/kanban-app/src/commands.rs: `resolve_board_root` (board root = parent of resolved .kanban, validated open via resolve_handle) + `#[tauri::command] expose_board_to_agents(state, board_path)` (resolves CLI via cli_install::resolve_bundled_cli(current_exe), spawn_blocking the inner fn). Reuses resolve_bundled_cli, returns a structured dev error when no CLI is bundled.
    - apps/kanban-app/src/main.rs: `mod expose;` + `commands::expose_board_to_agents` in generate_handler.
    - apps/kanban-app/Cargo.toml: dev-deps serial_test + mirdan test-support feature.

    Frontend:
    - ui/src/components/board-selector.tsx: a board-toolbar Pressable "Expose this board to your agent" (Share2 icon) next to the tear-off, gated `showTearOff && selectedPath`. Invokes `expose_board_to_agents` with `{boardPath}` and renders per-agent results as sonner toasts (success/error per agent; info toast when no agents detected).

    Tests:
    - apps/kanban-app/tests/expose_board.rs (NEW): #[path]-includes expose.rs; (1) temp board root + fake agent (MIRDAN_AGENTS_CONFIG) + CWD pinned elsewhere → asserts .mcp.json under the board root with absolute CLI path + args ["serve"], and nothing under CWD; (2) no-agents-detected → empty Vec + no .mcp.json.
    - ui board-selector.test.tsx: click expose button → asserts invoke("expose_board_to_agents", {boardPath}) + per-agent success/error toasts; + gating test.

    Adversarial double-check (really-done) found a real UX bug: register_mcp_server_at ALWAYS returns a non-empty summary Vec, so the inner fn never returned empty → zero-detected-agents showed a misleading green "Registered applied to 0 agent(s)" and the frontend's "no agents" info toast was dead code. FIXED: the fallback now surfaces only ERROR summaries and returns empty for the 0-agent success; regression test `expose_returns_empty_when_no_agents_detected` proves it (RED→GREEN).

    Verification (all green):
    - cargo nextest run -p kanban-app: 197 passed, 1 skipped
    - ui tsc --noEmit: exit 0
    - vitest browser board-selector.test.tsx: 10 passed
    - cargo fmt: clean; cargo clippy -p kanban-app --all-targets: only the two KNOWN pre-existing lints (shell.rs manual_contains, menu.rs empty_line_after_doc) — zero new warnings from this change.

    Leaving in `doing` for /review per the implement workflow.
  timestamp: 2026-06-27T03:07:04.609171+00:00
depends_on:
- 01KTVPZ1VE36FVG8CMQ49X8RMK
position_column: doing
position_ordinal: '8180'
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