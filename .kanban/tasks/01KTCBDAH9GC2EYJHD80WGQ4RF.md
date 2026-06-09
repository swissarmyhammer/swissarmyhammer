---
assignees:
- claude-code
position_column: todo
position_ordinal: '9280'
project: cli-schema-gen
title: 'DECISION: wire schema shape — compact per-op signatures vs bare op enum'
---
## What
Design decision (for the user) that gates the wire-schema slimming work (cards C, D). Decouples the heavy CLI-facing schema from what the MCP `tools/list` sends on every prompt.

Today every operation-based MCP tool returns the FULL schema over the wire via `McpTool::schema()`. For kanban that is ~25KB / ~6,300 tokens for 48 operations (`crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs:381` → `generate_kanban_mcp_schema` → `generate_mcp_schema` at `crates/swissarmyhammer-operations/src/schema.rs:82`). code_context's is larger. The heavy parts are `x-operation-schemas` (one full JSON-Schema sub-object per op, built by `operation_to_schema` at `schema.rs:190`), plus `x-operation-groups`, `x-forgiving-input`, and `examples`.

Verified facts that bound the decision:
- Nothing uses the wire schema for DISPATCH. `KanbanTool::execute` (`kanban/mod.rs:390`) calls the forgiving `parse_input` then `dispatch::execute_operation` in `crates/swissarmyhammer-kanban/src/dispatch.rs`. There is NO JSON-Schema validation. The wire schema is purely advisory to the model.
- The forgiving parser tolerates the model not knowing exact params/required fields (verb+noun+inferred input formats), so a slimmer wire schema does not break input handling.
- `x-operation-schemas` has exactly ONE production reader: kanban-cli's `apps/kanban-cli/src/cli_gen.rs` (`build_commands_from_schema:58`), and it obtains the schema IN-PROCESS (not over the wire) via `generate_kanban_mcp_schema` at `apps/kanban-cli/src/main.rs:34`.

Decide between two wire-schema shapes (the full schema is unchanged and stays available in-process to CLIs via `tool.operations()`):
- **Option 1 — compact per-op signatures (~3KB):** keep the `op` enum + tool description, and add a lightweight per-op signature map (op string → required field names, maybe types) so the model still sees which fields each op needs. Drops `x-operation-schemas`/`x-operation-groups`/`x-forgiving-input`/`examples`. More model guidance, modest token cost.
- **Option 2 — fully bare:** `op` enum + tool description only. Smallest possible. Relies entirely on the forgiving parser + the tool description prose for guidance.

Tradeoff to weigh: model guidance quality (will the agent supply the right params without per-op signatures?) vs token cost on every single prompt. Record the choice and the compact-signature format (if Option 1) so cards C and D can implement against a concrete target.

## Acceptance Criteria
- [ ] One option chosen and recorded in this card's description (append a `## Decision` section).
- [ ] If Option 1: the exact compact signature shape is specified (e.g. `{ "op-string": ["required_param", ...] }` vs including types/optionals) so C/D have an unambiguous target.
- [ ] The token/size budget target for the wire form is stated (used as the assertion target in card C).

## Tests
- [ ] No code in this card. The recorded decision becomes the spec that cards C and D test against (C asserts the wire form matches the chosen shape and size budget).

## Workflow
- Decision card, not a code change. Present the tradeoff to the user, capture the answer with the `question` tool, then write the `## Decision` section. Do not implement C/D until this is recorded.