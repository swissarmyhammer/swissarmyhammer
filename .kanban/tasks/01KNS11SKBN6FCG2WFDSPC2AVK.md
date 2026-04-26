---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffa380
project: kanban-mcp
title: 'kanban-cli: implement commands/serve.rs — KanbanMcpServer over stdio'
---
## What

Create `kanban-cli/src/commands/serve.rs` implementing a minimal `rmcp::ServerHandler` that exposes the single `kanban` operation tool over stdio.

Build directly on `swissarmyhammer-kanban` (already a dependency), NOT on `swissarmyhammer-tools::KanbanTool`.

Model error handling on shelltool's serve implementation. This file lives under `commands/` matching sah-cli's convention — command implementations go in `commands/`, infrastructure (cli.rs, banner.rs, logging.rs) stays top-level.

## Acceptance Criteria
- [x] `kanban-cli/src/commands/serve.rs` exists
- [x] `KanbanMcpServer` implements `ServerHandler` with `get_info`, `list_tools`, `call_tool`
- [x] `run_serve()` is pub async and returns `Result<(), String>`
- [x] `cargo check -p kanban-cli` passes

## Tests
- [x] Unit test: `KanbanMcpServer::get_info()` returns correct server name
- [x] Unit test: list_tools returns a single tool named `"kanban"`
- [x] Test file: `kanban-cli/src/commands/serve.rs` in `#[cfg(test)]` module

## Review Findings (2026-04-12 00:00)

### Warnings
- [x] `kanban-cli/src/commands/serve.rs` `call_tool` — dispatch errors are unconditionally mapped to `McpError::internal_error`, but many `KanbanError` variants (parse, not_found, validation, already_exists) are caller-facing and should surface as `invalid_params` or `invalid_request` so MCP clients can distinguish bad-input failures from server bugs. This mirrors an existing shape in `swissarmyhammer-tools::KanbanTool`, but the pattern is worth revisiting here — suggestion: match on `KanbanError` variant (or introduce a small helper) to classify errors before wrapping.
- [x] `kanban-cli/src/commands/serve.rs` `call_tool` — no test exercises the call dispatch path. `get_info` and `list_tools` are covered, but the non-trivial call_tool body (unknown-tool rejection, single vs batch result collapsing, parse-error → invalid_params mapping) has zero coverage. Add at least: (a) a test that calling a tool other than `"kanban"` yields `invalid_request`; (b) a test that a well-formed `"init board"` call (in a tempdir with CWD guarded) produces a success response; (c) a test that malformed input yields `invalid_params`.
- [x] `kanban-cli/src/commands/serve.rs` `call_tool` — the `swissarmyhammer-tools::KanbanTool` attaches ACP plan notifications (the `_plan` side channel) to task-modifying responses so agents can emit `session/update` plans. The CLI server drops that entirely. If this is intentional (stateless CLI, no plan sender), add a brief comment noting the omission and rationale so the next reader doesn't assume it was forgotten. If unintentional, the plan-building logic in `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs::build_plan_data` is pure and could be lifted into `swissarmyhammer-kanban` for shared use.

### Nits
- [x] `kanban-cli/src/commands/serve.rs` `KanbanMcpServer` — missing `Debug` impl. Per dtolnay Rust guidelines, all public types should implement `Debug`; a unit struct can derive it trivially. Add `Debug` to the `#[derive(...)]` list.
- [x] `kanban-cli/src/commands/serve.rs` `list_tools_returns_single_kanban_tool` test — the trailing `let _clone = server.clone();` is unrelated to the test's stated purpose (verifying the tool listing shape). Split into a dedicated `kanban_server_is_clone` test or drop it — rmcp's clone requirement is already enforced by the `#[derive(Clone)]` compiling.
- [x] `kanban-cli/src/commands/serve.rs` `SERVER_NAME` — used as both the MCP `Implementation` name and the tool name. The comment explains the intent, but two named constants (`SERVER_NAME` / `TOOL_NAME`) aliasing the same string would make the two semantic roles self-documenting and allow them to diverge cleanly in the future.
- [x] `kanban-cli/src/commands/serve.rs` `run_serve` — `#[allow(dead_code)]` is justified by the follow-up wiring card (`01KNS13JJ850R9NBA0RQCS7E9Z`), but add `#[allow(dead_code, reason = "wired up in <follow-up-card-id>")]` or equivalent when Rust 1.81+ stable-reasons are available project-wide, so the reason survives independently of the inline comment.

## Notes

All review findings addressed in a single pass (commit follows):

- **Error classification** — Added `classify_kanban_error()` helper that matches on `KanbanError` variants. Parse / MissingField / InvalidValue / InvalidOperation → `invalid_params`. NotInitialized / AlreadyExists / *NotFound / DuplicateId / DependencyCycle / ColumnNotEmpty / ProjectHasTasks → `invalid_request`. Lock / IO / JSON / YAML / FieldsError / ViewsError / StoreError → `internal_error`. `EntityError` is unwrapped and sub-classified (e.g. `EntityError::NotFound` → `invalid_request`, `EntityError::ValidationFailed` → `invalid_params`, `EntityError::Io` → `internal_error`) because some ops (notably `move task`) don't route through `from_entity_error` and would otherwise surface `EntityError::NotFound` as a server bug.
- **call_tool coverage** — Extracted a pure `dispatch_call_tool_request(ctx, request)` helper so tests can drive the dispatch path without constructing an rmcp `RequestContext<RoleServer>`. Added tests covering unknown-tool rejection (`invalid_request`), well-formed `init board` success, malformed op string (`invalid_params`), missing-task dispatch error (`invalid_request`), and CWD resolution via `CurrentDirGuard`. Also added direct unit tests for `classify_kanban_error` covering each error class and the `EntityError` sub-cases.
- **Plan notification comment** — Added a `# Plan notifications` section to the `dispatch_call_tool_request` doc-comment explaining the intentional omission of the ACP `_plan` side channel, and pointing at `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs::build_plan_data` as the lift target if plan support ever lands.
- **Debug derive** — Added `Debug` to `KanbanMcpServer`'s derive list.
- **Test split** — Moved the clone assertion into a dedicated `kanban_server_is_clone` test; `list_tools_returns_single_kanban_tool` now only asserts the tool-listing shape.
- **Constant split** — Split `SERVER_NAME` into `SERVER_NAME` (server Implementation identity) and `TOOL_NAME` (single exposed tool name); `TOOL_NAME` aliases `SERVER_NAME` today but is used everywhere the tool-name role is meant.
- **Allow reason** — `run_serve` now uses `#[allow(dead_code, reason = "wired up by follow-up card 01KNS13JJ850R9NBA0RQCS7E9Z (serve subcommand in main.rs)")]`.

Also added `swissarmyhammer-entity` as a direct dependency of `kanban-cli` because the `classify_kanban_error` helper needs to pattern-match on `EntityError` variants.

Verification: `cargo test -p kanban-cli` → 68 passed, 0 failed. `cargo clippy -p kanban-cli --tests --all-targets -- -D warnings` → clean.
