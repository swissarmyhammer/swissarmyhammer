---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: ai-panel
title: Add --board flag to kanban serve so it can target a board outside cwd
---
## What
`kanban serve` (`apps/kanban-cli/src/commands/serve.rs`) roots the kanban store at `<cwd>/.kanban` via `KanbanMcpServer::kanban_dir()`. The AI panel's TypeScript ACP client passes the kanban tool to the agent as a stdio MCP server entry in the ACP `newSession.mcpServers` array — `command` = the bundled `kanban`, `args` = `["serve", "--board", <board dir>]`. The agent spawns it. So `kanban serve` must accept an explicit board directory rather than relying on cwd.

- Add a `--board <PATH>` argument to the `serve` subcommand (`apps/kanban-cli/src/cli.rs` and `commands/serve.rs`).
- Thread it into `KanbanMcpServer` so it resolves `.kanban` from the given path (reuse `resolve_kanban_path`-style logic). No flag → unchanged cwd behavior.
- This is the only change `kanban serve` needs — the existing server and `kanban` tool are reused as-is, nothing extracted or rebuilt.

## Acceptance Criteria
- [ ] `kanban serve --board /path/to/project` serves the board at that path regardless of process cwd.
- [ ] No flag → unchanged cwd behavior.
- [ ] `cargo build -p kanban-cli` clean; existing `serve` tests pass.

## Tests
- [ ] Integration test: run `kanban serve --board <tempdir>` (or drive `KanbanMcpServer` with an explicit path), call the `kanban` tool with `op: "add task"`, assert the task lands in `<tempdir>/.kanban`.
- [ ] Keep the existing `serve.rs` tests green.
- [ ] `cargo test -p kanban-cli` is green.

## Workflow
- Use `/tdd` — write the `--board` targeting test first.