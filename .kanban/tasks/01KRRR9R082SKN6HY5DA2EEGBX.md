---
assignees:
- claude-code
depends_on:
- 01KRRR9APW3ETN2QWB29ZFZ60T
position_column: todo
position_ordinal: 8f80
project: ai-panel
title: Serve the full SwissArmyHammer MCP toolset in-process, per board
---
## What
The agent gets the **full SAH MCP toolset** (kanban, skills/prompts, code-context, files, etc.) — not just the `kanban` tool. `swissarmyhammer-tools` already serves the full toolset over HTTP in-process — reuse it, build/extract nothing.

- Add `swissarmyhammer-tools` to `apps/kanban-app/Cargo.toml`.
- Per open board (`BoardHandle`, `apps/kanban-app/src/state.rs`): start an MCP server via `swissarmyhammer_tools::mcp::start_mcp_server(McpServerMode::Http { port: Some(0) }, .., working_dir, ..)` with `working_dir` = the board folder. It binds a random loopback port; `McpServerHandle::url()` is `http://127.0.0.1:<port>/mcp`.
- Rooting at the board folder means its `kanban` tool operates on that board's `.kanban`, and skills/prompts resolve from that board's SAH directory (created by the inline-`sah init` task).
- Hold the `McpServerHandle` on `BoardHandle`; call `shutdown()` when the board closes (extend `Drop for BoardHandle`).
- Expose the board's MCP URL to the backend (consumed by `ai_start_agent`).
- Note: `swissarmyhammer-tools` is a large crate — this is a deliberate build-weight cost that the full-toolset requirement implies; acceptable per the product decision.

## Acceptance Criteria
- [ ] Each open board runs an in-process full-SAH-toolset MCP server on a loopback HTTP URL, rooted at the board folder.
- [ ] The server exposes the full toolset — an MCP `tools/list` includes `kanban` and the other SAH tools.
- [ ] Closing a board shuts its MCP server down — no leaks.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Integration test: open a board, connect an MCP client to the board's URL, assert `tools/list` includes `kanban` and at least one other SAH tool; a `kanban` `add task` call mutates that board; closing the board stops the server.
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the `tools/list` + board-mutation integration test first.