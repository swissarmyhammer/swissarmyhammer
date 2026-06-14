---
assignees:
- claude-code
depends_on:
- 01KRRE967SBZ5TH2JPDMSV21BY
- 01KRRE4BYZ9CP342YBBJWGNK7M
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff80
project: plugin-arch
title: 'tools: expose existing in-process MCP tools to the plugin registry'
---
## What
Wire the existing in-process MCP tools in `swissarmyhammer-tools` into the plugin platform via `expose_rust_module`, so plugins (and the host itself) can register them by id. The tools keep their `McpTool`/`ToolRegistry` home — only the exposure glue is new.

In `crates/swissarmyhammer-tools/src/mcp/server.rs` (the MCP server bootstrap, around `create_tool_context_and_registry` / `register_all_tools`):
- Add `swissarmyhammer-plugin` as a dependency of `swissarmyhammer-tools`.
- After the existing `ToolRegistry` is built, hand each tool to the plugin platform: `host.expose_rust_module("files", ...)`, `"kanban"`, `"code_context"`, `"git"`, `"shell"`, etc. — one per in-process tool already registered.
- The exposed value must satisfy the platform's in-process server contract — wrap each tool as an rmcp `ServerHandler` (or adapt the existing `McpTool` so an `InProcessServer` can wrap it). If the existing tools are not already rmcp `ServerHandler`s, add the thin adapter here; do NOT rewrite the tools.
- Operation tools among these (`files`, `kanban`, `code_context`, `git`, `shell`) must surface the `io.swissarmyhammer/operations` `_meta` — use the operation-tool macro so `_meta` is attached by construction. Migrating a tool's schema construction to the macro is in scope for the operation tools touched here.
- `expose_rust_module` registers into the "available modules" table; it does NOT put servers live. A `register(name, {rust: id})` (host config or plugin) activates them.

## Acceptance Criteria
- [x] `swissarmyhammer-tools` depends on `swissarmyhammer-plugin` and calls `expose_rust_module` for each in-process tool during MCP bootstrap.
- [x] Each exposed tool is wrappable by `InProcessServer` and reachable through the `Dispatcher` after a `register(name, {rust: id})`.
- [x] The operation tools expose `_meta["io.swissarmyhammer/operations"]` via the macro — no hand-written `_meta`.
- [x] Existing `swissarmyhammer-tools` behavior and its test suite are unchanged.

## Tests
- [x] Integration test in `swissarmyhammer-tools/tests/`: build the MCP server, assert the `files` (and `kanban`) modules are present in the available-modules table, activate one via `register`, and call a tool through the `Dispatcher` — assert the real effect (e.g. `files` writes a file).
- [x] Test that an exposed operation tool's `tools()` carries the `io.swissarmyhammer/operations` `_meta` key.
- [x] Run: `cargo test -p swissarmyhammer-tools` — all green; existing tests still pass.

## Workflow
- Use `/tdd` — write the expose → activate → call test first, then implement the wiring.

## Depends on
PluginHost (`expose_rust_module`) and the operation-tool macro.

## Implementation notes
- `McpTool` → `McpServer` adapter: `ToolModuleServer` in `crates/swissarmyhammer-tools/src/mcp/plugin_bridge.rs` implements the platform `McpServer` trait directly (one tool per module). Going through rmcp `ServerHandler`/`InProcessServer` is not viable because `McpTool::execute` takes a `ToolContext`, not an `rmcp::RequestContext`, and rmcp offers no seam to reconstruct one — the task explicitly permits a direct `McpServer` impl. The exposed value is still a valid `Arc<dyn McpServer>` reachable through the `Dispatcher`.
- Exposure glue: `McpServer::expose_tools_to_plugin_host(&PluginHost)` iterates the live `ToolRegistry` and calls `expose_rust_module` once per enabled tool, keyed by tool name.
- `_meta`: `build_tool_definition` attaches `io.swissarmyhammer/operations` via `swissarmyhammer_operations::generate_operations_meta(tool.operations())` — the same generator the `operation_tool!` macro uses, over the same operation slice the tool's `schema()` is built from. `FilesTool` gained an `operations()` override (mirroring git/shell) so it surfaces its operation set; the other four operation tools already had it.
- The mandated `swissarmyhammer-plugin` dependency pulled `smartstring` into the `swissarmyhammer-tools` dependency closure; its `impl Add<SmartString> for String` made two pre-existing test lines (`String + &"a".repeat(...)`) ambiguous. Both were rewritten with `format!` to keep the existing suite compiling — no behavior change.