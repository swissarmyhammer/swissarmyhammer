---
assignees:
- claude-code
depends_on:
- 01KRRR9APW3ETN2QWB29ZFZ60T
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff280
project: ai-panel
title: Serve the full SwissArmyHammer MCP toolset in-process, per board
---
## What
The agent gets the **full SAH MCP toolset** (kanban, skills/prompts, code-context, files, etc.) — not just the `kanban` tool. `swissarmyhammer-tools` already serves the full toolset over HTTP in-process — reuse it, build/extract nothing.

- Add `swissarmyhammer-tools` to `apps/kanban-app/Cargo.toml`.
- Per open board (`BoardHandle`, `apps/kanban-app/src/state.rs`): start an MCP server via `swissarmyhammer_tools::mcp::start_mcp_server(McpServerMode::Http { port: None }, .., working_dir, ..)` with `working_dir` = the board folder. It binds a random loopback port; `McpServerHandle::url()` is `http://127.0.0.1:<port>/mcp`.
- Rooting at the board folder means its `kanban` tool operates on that board's `.kanban`, and skills/prompts resolve from that board's SAH directory (created by the inline-`sah init` task).
- Hold the `McpServerHandle` on `BoardHandle`; call `shutdown()` when the board closes (extend `Drop for BoardHandle`).
- Expose the board's MCP URL to the backend (consumed by `ai_start_agent`).
- Note: `swissarmyhammer-tools` is a large crate — this is a deliberate build-weight cost that the full-toolset requirement implies; acceptable per the product decision.

## Acceptance Criteria
- [x] Each open board runs an in-process full-SAH-toolset MCP server on a loopback HTTP URL, rooted at the board folder.
- [x] The server exposes the full toolset — an MCP `tools/list` includes `kanban` and the other SAH tools.
- [x] Closing a board shuts its MCP server down — no leaks.
- [x] `cargo build -p kanban-app` is clean.

## Tests
- [x] Integration test: open a board, connect an MCP client to the board's URL, assert `tools/list` includes `kanban` and at least one other SAH tool; a `kanban` `add task` call mutates that board; closing the board stops the server.
- [x] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the `tools/list` + board-mutation integration test first.

## Implementation Notes

**Deviation from the literal spec — `port: None`, not `Some(0)` (user-approved).**
The description specified `McpServerMode::Http { port: Some(0) }`. Verified against the real `start_mcp_server` source: `resolve_http_port` only asks the OS for an ephemeral port when the variant is `Http { port: None }`. With `Some(0)` it returns `0` verbatim, so `connection_url`/`url()` would be literally `http://127.0.0.1:0/mcp` — an unusable URL (the TDD test confirmed: client failed with "Can't assign requested address"). `Http { port: None }` produces exactly the task's stated intent: a random loopback port and a correct `http://127.0.0.1:<realport>/mcp` URL. Asked the user before deviating; approved (`.sah/questions/20260517_115641_110752_question.yaml`).

**Full toolset — `agent_mode = true` (resolves review Warning 1, 2026-05-17).**
The first implementation called `start_mcp_server(..)`, which delegates to `start_mcp_server_with_options(.., agent_mode = false)`. With `agent_mode = false`, `McpServer::register_all_tools` calls `tool_registry.remove_agent_tools()`, stripping every `is_agent_tool()` tool — the `skill` tool (`tools/skill/mod.rs:218`), the `web` tool (`tools/web/mod.rs:63`), and the full-access `files` tool (`tools/files/mod.rs:130-132`; only the read-only variant survives). That directly contradicts the task: serve the **full** toolset — "kanban, skills/prompts, code-context, files, etc." — and the board-folder rooting exists precisely so "skills/prompts resolve from that board's SAH directory", which the `skill` tool consumes. Fix: `start_board_mcp_server` now calls `start_mcp_server_with_options(McpServerMode::Http { port: None }, None, None, Some(board_dir), true)` — the public agent-mode-aware entry point in `swissarmyhammer-tools/src/mcp/unified_server.rs:616`. `agent_mode = true` registers the agent tools, so `tools/list` carries `kanban`, `git`, `code_context`, `skill`, `files`, and `web`. The full toolset is now served; no spec divergence remains.

**Per-board MCP server lifecycle.**
- New free fn `start_board_mcp_server(kanban_path)` in `state.rs` derives the board folder via `kanban_path.parent()` (same math as `ensure_sah_workspace`) and starts an HTTP MCP server rooted there. Failures are logged and swallowed so a port/filesystem problem never blocks a board from opening.
- `BoardHandle` gained an `mcp_server: Option<McpServerHandle>` field, started in `BoardHandle::open` after the search index loads.
- `BoardHandle::mcp_url() -> Option<&str>` exposes the URL to the AI backend. It carries a targeted `#[allow(dead_code)]` with a documented rationale: the production call site is the follow-up task `01KRRN3SP5D1H63TQ8HM7SQZ1F` (`ai_start_agent`), which this task blocks. This mirrors the existing `#![allow(dead_code)]` on `ai/mod.rs`. The board-lifecycle integration test exercises the accessor.
- `Drop for BoardHandle` extended: takes the `McpServerHandle` and spawns the async `shutdown()` onto the current Tokio runtime when one is reachable; otherwise dropping the handle still fires `Drop for McpServerHandle`, which best-effort sends the same shutdown signal. The server stops either way — no leaked loopback port.

**Test surface.** `kanban-app` is binary-only (no lib target), so the board-lifecycle integration test lives in `state.rs`'s `#[cfg(test)] mod tests` — the same in-binary integration surface used by `test_open_board_creates_sah_workspace_at_board_folder`. `rmcp` + `reqwest` were added as dev-dependencies; the rmcp HTTP client pattern mirrors `swissarmyhammer-tools/tests/integration/final_http.rs`. After review Warning 2, `test_open_board_serves_full_sah_mcp_toolset` asserts the specific expected `McpTool::name()` strings — `kanban`, `git`, `code_context`, `skill`, `files`, `web` — so a regression in tool registration or a flip back to `agent_mode = false` is actually caught (the old "at least one non-kanban tool" assertion could not detect tool-stripping).

**Verification.** `cargo build -p kanban-app` clean; `cargo clippy -p kanban-app --all-targets -- -D warnings` clean; `cargo test -p kanban-app` green.

## Review Findings (2026-05-17 13:30)

### Warnings
- [x] `apps/kanban-app/src/state.rs:1091-1092` — `start_board_mcp_server` calls `start_mcp_server`, which hardcodes `agent_mode = false` (it delegates to `start_mcp_server_with_options(.., false)`). With `agent_mode = false`, `McpServer::register_all_tools` calls `tool_registry.remove_agent_tools()`, which strips every tool whose `is_agent_tool()` returns `true` — confirmed to be the `skill` tool (`tools/skill/mod.rs:218`), the `web` tool (`tools/web/mod.rs`), and the full-access `files` tool (`tools/files/mod.rs:130-132`; only the read-only `files` variant survives). The task description explicitly lists the intended toolset as "kanban, **skills/prompts**, code-context, **files**, etc." and states the board-folder rooting exists so "skills/prompts resolve from that board's SAH directory" — but the `skill` tool that would consume that directory is removed by this mode choice. The retained `agent` tool covers *prompts*, not *skills*. This may well be the correct call (the per-board server supplements a Claude-Code-style agent that has files/skill natively — exactly the documented purpose of `agent_mode = false`), but it diverges from the literal task spec and is not called out as a deliberate decision the way the `port` deviation was. Either (a) switch to `start_mcp_server_with_options(.., agent_mode = true)` so the `skill`/`web`/full-`files` tools are served, or (b) document the `agent_mode = false` choice as a sanctioned deviation in the Implementation Notes (mirroring the `port: None` note), explaining that the AI panel agent supplies those agent-side tools itself. Surface this to the product owner — it is the same class of spec divergence as the approved `port` change and should be an explicit decision, not an implicit one. *RESOLVED: chose option (a). `start_board_mcp_server` now calls `start_mcp_server_with_options(.., agent_mode = true)` so the full toolset (`skill`, `web`, full-`files`) is served — see the "Full toolset — `agent_mode = true`" Implementation Note.*
- [x] `apps/kanban-app/src/state.rs:1698-1701` — The integration test asserts only that `tools/list` carries `kanban` plus *at least one* tool whose name `!= "kanban"`. That assertion passes regardless of which agent tools were stripped, so it cannot detect the `skill`/`web`/full-`files` removal noted above and gives no real coverage of the "full toolset" acceptance criterion. Tighten the test to assert the specific tools the description requires (e.g. `code-context`, `git`, and — depending on the resolution of the warning above — `skill`/`files`), so a future regression in tool registration or `agent_mode` is actually caught. *RESOLVED: `test_open_board_serves_full_sah_mcp_toolset` now asserts `tools/list` contains each of `kanban`, `git`, `code_context`, `skill`, `files`, `web` (the exact `McpTool::name()` strings the server registers).*

### Nits
- [x] `ARCHITECTURE.md:543` — The `kanban-app` section documents `BoardHandle` as "wrapping KanbanContext + StoreContext + EntityCache + SearchIndex". This task adds a fifth component — a per-board in-process MCP server (`mcp_server: Option<McpServerHandle>`). Extend that sentence so the documented `BoardHandle` composition matches the code. *RESOLVED: the sentence now reads "... EntityCache + SearchIndex + a per-board in-process full-SAH-toolset MCP server".*