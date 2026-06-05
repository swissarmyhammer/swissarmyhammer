---
assignees:
- claude-code
depends_on:
- 01KTC9VVAW145DZ7626BCHRRPG
position_column: todo
position_ordinal: '9080'
project: semantic-search
title: 'Register search tasks: Verb::Search, dispatch arm, validity table, schema'
---
## What
Wire the new `SearchTasks` op (previous card) into the kanban operation surface so `{"op":"search tasks", ...}` parses, dispatches, validates, and appears in the generated MCP schema. The kanban MCP tool (`crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`) dispatches generically through `parse_input` -> `execute_operation` and pulls its schema from the kanban crate's single source of truth, so ALL changes are in `crates/swissarmyhammer-kanban/`.

CRITICAL collision: today `"search"` is an ALIAS for `Verb::List` (`crates/swissarmyhammer-kanban/src/types/operation.rs`, `Verb::from_alias`). A `search tasks` op would currently parse to `(List, Tasks)` and collide with `list tasks`. So add a distinct verb:
1. `crates/swissarmyhammer-kanban/src/types/operation.rs`:
   - Add `Search` to `enum Verb`; add `Self::Search => "search"` to `as_str`.
   - In `from_alias`, REMOVE `"search"` from the `List` arm and add a new arm `"search" => Some(Self::Search)`. (`"find" | "query"` may stay on `List` or move — keep `list`/`ls` on List; `search` becomes its own verb. Document the choice.)
   - In `is_valid_operation` (the `(Verb, Noun)` validity table) add `(Verb::Search, Noun::Tasks)` (and `Noun::Task` if singular is accepted) as valid. Update the test asserting valid/invalid pairs.
2. `crates/swissarmyhammer-kanban/src/dispatch.rs`:
   - In `execute_task_query_operation` (the `Verb::Next | Verb::List` handler) add a `Verb::Search` arm that builds `SearchTasks` from params: `query` (required via `req`), optional `filter`, optional `top_k` (via `op.get_u64`), then `processor.process(&cmd, ctx)`.
   - In `execute_task_operation`, add `Verb::Search` to the set routed to `execute_task_query_operation`.
   - Import `SearchTasks` in the `crate::task::{…}` use list.
3. `crates/swissarmyhammer-kanban/src/schema.rs`:
   - Add `Box::leak(Box::new(SearchTasks::new()))` (or the appropriate constructor) to `KANBAN_OPERATIONS` in the Task section so `search tasks` appears in the generated MCP `x-operation-schemas` with its `query`/`filter`/`top_k` params. Import `SearchTasks`.
   - If the kanban schema declares verb aliases (the schema module mentions kanban-tailored aliases), ensure `search` maps to the search verb, not list.

Verify the MCP tool needs NO change: confirm `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` dispatches via `execute_operation` and derives schema from `kanban` crate — if so, the new op flows through automatically once the crate changes land.

## Acceptance Criteria
- [ ] `{"op":"search tasks","query":"x"}` parses to `(Verb::Search, Noun::Tasks)`, NOT `(List, Tasks)`.
- [ ] `"list tasks"` and `"ls tasks"` still parse to `(List, Tasks)` and behave unchanged.
- [ ] `is_valid_operation(Verb::Search, Noun::Tasks)` is true.
- [ ] Dispatching `search tasks` executes `SearchTasks` and returns ranked task JSON.
- [ ] The generated kanban MCP schema lists a `search tasks` operation with `query`, `filter`, `top_k`.
- [ ] `cargo build -p swissarmyhammer-kanban` and `cargo build -p swissarmyhammer-tools` compile.

## Tests
- [ ] In `crates/swissarmyhammer-kanban/src/dispatch.rs` `#[cfg(test)]`: a `parse_input(json!({"op":"search tasks","query":"..."}))` -> `execute_operation` test (pattern from `dispatch_add_and_list_tasks`) asserting the op runs and returns a `tasks` array; assert `list tasks` is unaffected.
- [ ] In `operation.rs` tests: `Verb::from_alias("search") == Some(Verb::Search)`; `Verb::from_alias("ls") == Some(Verb::List)`; `is_valid_operation(Verb::Search, Noun::Tasks)` true.
- [ ] A schema test asserting the generated op list contains `search tasks` (mirror the existing kanban schema-generation test if present).
- [ ] `cargo test -p swissarmyhammer-kanban dispatch` and `cargo test -p swissarmyhammer-kanban operation` pass.

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.