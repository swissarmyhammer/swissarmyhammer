---
assignees:
- claude-code
depends_on:
- 01KTCBEGPQ04987DT6PHA6CNTD
position_column: todo
position_ordinal: '9580'
project: cli-schema-gen
title: Point every McpTool::schema() wire output at the slim variant
---
## What
Re-point the wire-facing `McpTool::schema()` of every operation-based tool at the slim wire generator from card C, while `operations()` continues to return the full op list (CLIs build the full schema in-process from `tool.operations()`). Dispatch is untouched — `execute` still uses the forgiving `parse_input`.

Per-tool wrappers each call the shared generator; switch each to the slim variant (keep a `_full` wrapper where a card/test still needs the full schema in-process):
- `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:381` → `generate_kanban_mcp_schema` (`crates/swissarmyhammer-kanban/src/schema.rs:97`)
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1122` → `code_context/schema.rs` (`generate_..._mcp_schema:13`)
- `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:502` → direct `generate_mcp_schema(&SHELL_OPERATIONS, ...)` at line 506
- `crates/swissarmyhammer-tools/src/mcp/tools/files/mod.rs:117` → `files/schema.rs:7` (`generate_files_mcp_schema`)
- `crates/swissarmyhammer-tools/src/mcp/tools/web/mod.rs:55` → `web/schema.rs:7`
- `crates/swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs:261` → direct `generate_mcp_schema` at line 265
- `crates/swissarmyhammer-tools/src/mcp/tools/questions/mod.rs:56` → `questions/schema.rs:7`

Same-pattern tools NOT in the brief's named seven but built identically — apply the same change for consistency (the wire bloat affects them equally): `ralph/execute/mod.rs:213`, `agent/mod.rs:154`, `skill/mod.rs:139`. If any has a reason to keep the full schema on the wire, note it in the card.

Mechanically: where a wrapper exists, change its body to call the slim generator (and add a sibling `_full` wrapper for in-process callers if not already present from card C). Where `schema()` calls `generate_mcp_schema` directly, swap to the slim fn.

## Acceptance Criteria
- [ ] Every listed tool's `schema()` returns the slim wire schema (no `x-operation-schemas`/`x-operation-groups`/`x-forgiving-input`/`examples`).
- [ ] `operations()` still returns the full op slice for every tool (CLIs unaffected).
- [ ] `execute`/dispatch paths unchanged; full workspace builds.

## Tests
- [ ] Update the per-tool schema unit tests that currently assert full-schema keys: `files/schema.rs` (tests at :67+), `web/schema.rs` (:40+), `shell/mod.rs` schema tests, `code_context/schema.rs`, kanban `crates/swissarmyhammer-kanban/src/schema.rs` (:207+), `questions/mod.rs`, `git/changes/mod.rs`. Split each: full-schema tests target the `_full` wrapper; wire tests assert the slim `schema()` omits the heavy keys.
- [ ] Move the integration assertion in `apps/swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs` (test `test_kanban_schema_has_all_operations`, :202-258): the `x-operation-schemas` count assertion (:224-235) must move to the FULL-schema path (`generate_kanban_mcp_schema_full` / `tool.operations()`), while the wire `schema()` is asserted to OMIT `x-operation-schemas`. Keep the op-enum count assertion (:215-222) on the wire schema.
- [ ] `cargo nextest run -p swissarmyhammer-tools schema` and the `mcp_tools_registration` integration test pass.

## Workflow
- Use `/tdd` — flip the test expectations first (wire omits heavy keys, full keeps them), watch them fail, then re-point the `schema()` methods.