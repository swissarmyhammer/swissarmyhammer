---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
position_column: todo
position_ordinal: ab80
project: kanban-mcp
title: 'kanban-cli: implement commands/serve.rs — KanbanMcpServer over stdio'
---
## What

Create `kanban-cli/src/commands/serve.rs` implementing a minimal `rmcp::ServerHandler` that exposes the single `kanban` operation tool over stdio.

Build directly on `swissarmyhammer-kanban` (already a dependency), NOT on `swissarmyhammer-tools::KanbanTool`.

Model error handling on shelltool's serve implementation. This file lives under `commands/` matching sah-cli's convention — command implementations go in `commands/`, infrastructure (cli.rs, banner.rs, logging.rs) stays top-level.

## Acceptance Criteria
- [ ] `kanban-cli/src/commands/serve.rs` exists
- [ ] `KanbanMcpServer` implements `ServerHandler` with `get_info`, `list_tools`, `call_tool`
- [ ] `run_serve()` is pub async and returns `Result<(), String>`
- [ ] `cargo check -p kanban-cli` passes

## Tests
- [ ] Unit test: `KanbanMcpServer::get_info()` returns correct server name
- [ ] Unit test: list_tools returns a single tool named `"kanban"`
- [ ] Test file: `kanban-cli/src/commands/serve.rs` in `#[cfg(test)]` module
