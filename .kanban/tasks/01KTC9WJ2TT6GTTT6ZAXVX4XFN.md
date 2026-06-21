---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv6d4gs7yat1xbpw92kd749d
  text: 'Picked up by /finish (scoped-batch $semantic-search). Dependency ^bchrrpg is done. IMPORTANT SCOPE OVERLAP: the bchrrpg implementer ALREADY landed most of this card''s wiring because it was needed to make the op dispatchable — Verb::Search added (as_str, from_alias moved "search" off List), is_mutation (read-only), is_valid_operation((Search,Tasks)); dispatch.rs Verb::Search arm in execute_task_query_operation + routing + dispatch_search_tasks_wiring test; schema.rs KANBAN_OPERATIONS registration + removed "search" from list alias doc. So this card is mostly VERIFY-against-AC + fill TEST GAPS: confirm find/query stay on List (back-compat), (Search, non-task-noun) is invalid, list/ls/find/query tasks still → (List,Tasks), and add the schema test asserting the generated op list contains `search tasks` with query/filter/top_k if not already present. Have the implementer audit current tree vs this card''s AC and add only what''s missing. Delegating to /implement (TDD).'
  timestamp: 2026-06-15T19:46:32.359912+00:00
- actor: claude-code
  id: 01kv6dd0b102f090pgp7gmf9gn
  text: |-
    AUDIT RESULT: All production wiring was already landed by ^bchrrpg. Confirmed present and unchanged:
    - operation.rs: Verb::Search variant + as_str("search") + from_alias("search"=>Search, find/query/ls kept on List) + is_mutation (Search read-only) + is_valid_operation((Search,Tasks)).
    - dispatch.rs: Verb::Search arm in execute_task_query_operation (query req, optional filter/top_k) + routing in execute_task_operation + dispatch_search_tasks_wiring test (parse->execute->tasks array, missing query errors).
    - schema.rs: SearchTasks registered in KANBAN_OPERATIONS + imported.
    - MCP tool (swissarmyhammer-tools/.../kanban/mod.rs): CONFIRMED needs NO change — dispatches generically via swissarmyhammer_kanban::dispatch::execute_operation and derives schema from kanban_operations(). New op flows through automatically.

    WHAT I ADDED (test gaps only, TDD):
    - operation.rs: test_search_alias_distinct_from_list (search=>Search; list/ls/find/query=>List) and test_search_is_valid_only_for_tasks ((Search,Tasks) valid; (Search, Task/Board/Actors/Projects/Tags) invalid).
    - schema.rs: test_schema_includes_search_tasks_with_params — asserts op enum (both wire+full surfaces) contains "search tasks" AND "list tasks" unaffected, and full schema's x-operation-schemas entry for "search tasks" exposes query/filter/top_k params. Red-checked: removing SearchTasks from KANBAN_OPERATIONS makes it FAIL ("op enum should contain 'search tasks'"), then restored.

    VERIFICATION (all green):
    - cargo test -p swissarmyhammer-kanban dispatch: 103 passed; 0 failed (incl dispatch_search_tasks_wiring).
    - cargo test -p swissarmyhammer-kanban operation: 24 passed; 0 failed.
    - cargo test -p swissarmyhammer-kanban schema: 17 passed; 0 failed (incl test_schema_includes_search_tasks_with_params).
    - cargo build -p swissarmyhammer-kanban: Finished, ok. cargo build -p swissarmyhammer-tools: Finished, ok.
    - cargo clippy -p swissarmyhammer-kanban --all-targets -- -D warnings: Finished, 0 warnings.

    Net: bchrrpg landed all production wiring; this card filled the three required TEST gaps. Moving to review.
  timestamp: 2026-06-15T19:51:10.433341+00:00
- actor: claude-code
  id: 01kv6ddxzq67b0mm5203fvde4x
  text: '/implement done — task in review, all AC checked. AUDIT confirmed bchrrpg already landed ALL production wiring (operation.rs Verb::Search/as_str/from_alias["search"→Search; find/query/ls kept on List]/is_mutation/is_valid_operation((Search,Tasks)); dispatch.rs Verb::Search arm + routing + dispatch_search_tasks_wiring test; schema.rs SearchTasks in KANBAN_OPERATIONS). swissarmyhammer-tools kanban/mod.rs CONFIRMED needs NO change — dispatches generically via execute_operation + derives schema from kanban_operations(), op flows through automatically. ADDED test gaps only (TDD): operation.rs test_search_alias_distinct_from_list + test_search_is_valid_only_for_tasks; schema.rs test_schema_includes_search_tasks_with_params (RED-verified by removing SearchTasks from KANBAN_OPERATIONS). Verify: dispatch 103 passed, operation 24 passed, schema 17 passed, build kanban + tools clean, clippy -D warnings clean. Proceeding to /review.'
  timestamp: 2026-06-15T19:51:40.791660+00:00
- actor: claude-code
  id: 01kv6dx4b5rjz3dgcks43wj0dt
  text: '/review done (run INCOMPLETE 1/15 but ground-truth-verified): 0 blockers, 0 warnings, 2 nits — both pre-existing/out-of-surface (hardcoded 10 in pre-existing test_full_schema_has_perspective_examples not this card''s added test; is_mutation matches! design critique predates this card, one-token Verb::Search addition is minimal correct edit). Reviewer confirmed the 3 added tests are load-bearing not over-fit: test_search_alias_distinct_from_list (search→Search; list/ls/find/query stay List), test_search_is_valid_only_for_tasks ((Search,Tasks) valid; Search+non-task invalid), test_schema_includes_search_tasks_with_params (both schema surfaces advertise search tasks + query/filter/top_k). AC satisfied by current tree; MCP-tool no-change claim correct. Moved to done.'
  timestamp: 2026-06-15T19:59:58.821298+00:00
depends_on:
- 01KTC9VVAW145DZ7626BCHRRPG
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb080
project: semantic-search
title: 'Register search tasks: Verb::Search, dispatch arm, validity table, schema'
---
## What
Wire the new `SearchTasks` op (previous card) into the kanban operation surface so `{"op":"search tasks", ...}` parses, dispatches, validates, and appears in the generated MCP schema. The kanban MCP tool (`crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`) dispatches generically through `parse_input` -> `execute_operation` and pulls its schema from the kanban crate's single source of truth, so ALL changes are in `crates/swissarmyhammer-kanban/`.

REFERENCE IMPLEMENTATIONS (this exact pattern already exists): `swissarmyhammer-skills` (`search skill`) and `swissarmyhammer-agents` (`search agent`) both added a `Verb::Search` + parse + dispatch + schema. Mirror `crates/swissarmyhammer-skills/src/parse.rs` / `schema.rs` and `crates/swissarmyhammer-agents/src/parse.rs` / `schema.rs`. `search <noun>` meaning a relevance search is the established codebase convention (`search skill`, `search agent`, `search symbol`, `search code`, `search url`) — kanban's `"search"`-as-a-`List`-alias is the odd one out; this card aligns it.

CRITICAL collision: today `"search"` is an ALIAS for `Verb::List` (`crates/swissarmyhammer-kanban/src/types/operation.rs`, `Verb::from_alias`). A `search tasks` op would currently parse to `(List, Tasks)` and collide with `list tasks`. VERIFIED SAFE: a repo-wide grep found NO caller using `search <kanban-noun>` as a list alias (all `search *` usages are other tools' own ops), so moving `"search"` off `List` breaks nothing. Add a distinct verb:
1. `crates/swissarmyhammer-kanban/src/types/operation.rs`:
   - Add `Search` to `enum Verb`; add `Self::Search => "search"` to `as_str`.
   - In `from_alias`, REMOVE `"search"` from the `List` arm and add a new arm `"search" => Some(Self::Search)`. KEEP `"find"` and `"query"` on `List` (back-compat: any alias-based "list" use of `find`/`query` for any noun keeps working; only `search` moves). Keep `list`/`ls` on List. Document the choice in a comment.
   - In `is_valid_operation` (the `(Verb, Noun)` validity table) add `(Verb::Search, Noun::Tasks)` (and `Noun::Task` if singular is accepted) as valid. `Verb::Search` is valid ONLY with tasks — `search <other-noun>` is intentionally invalid (nothing uses it; `find`/`query` remain for list-via-alias). Update the test asserting valid/invalid pairs.
2. `crates/swissarmyhammer-kanban/src/dispatch.rs`:
   - In `execute_task_query_operation` (the `Verb::Next | Verb::List` handler) add a `Verb::Search` arm that builds `SearchTasks` from params: `query` (required via `req`), optional `filter`, optional `top_k` (via `op.get_u64`), then `processor.process(&cmd, ctx)`.
   - In `execute_task_operation`, add `Verb::Search` to the set routed to `execute_task_query_operation`.
   - Import `SearchTasks` in the `crate::task::{…}` use list.
3. `crates/swissarmyhammer-kanban/src/schema.rs`:
   - Add `Box::leak(Box::new(SearchTasks::new()))` (or the appropriate constructor) to `KANBAN_OPERATIONS` in the Task section so `search tasks` appears in the generated MCP `x-operation-schemas` with its `query`/`filter`/`top_k` params. Import `SearchTasks`.
   - If the kanban schema declares verb aliases, ensure `search` maps to the search verb, not list.

Verify the MCP tool needs NO change: confirm `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` dispatches via `execute_operation` and derives schema from the `kanban` crate — if so, the new op flows through automatically once the crate changes land.

## Acceptance Criteria
- [ ] `{"op":"search tasks","query":"x"}` parses to `(Verb::Search, Noun::Tasks)`, NOT `(List, Tasks)`.
- [ ] `"list tasks"`, `"ls tasks"`, `"find tasks"`, `"query tasks"` still parse to `(List, Tasks)` and behave unchanged (find/query kept on List).
- [ ] `is_valid_operation(Verb::Search, Noun::Tasks)` is true; `(Verb::Search, <non-task noun>)` is invalid.
- [ ] Dispatching `search tasks` executes `SearchTasks` and returns ranked task JSON.
- [ ] The generated kanban MCP schema lists a `search tasks` operation with `query`, `filter`, `top_k`.
- [ ] `cargo build -p swissarmyhammer-kanban` and `cargo build -p swissarmyhammer-tools` compile.

## Tests
- [ ] In `crates/swissarmyhammer-kanban/src/dispatch.rs` `#[cfg(test)]`: a `parse_input(json!({"op":"search tasks","query":"..."}))` -> `execute_operation` test (pattern from `dispatch_add_and_list_tasks`) asserting the op runs and returns a `tasks` array; assert `list tasks` is unaffected.
- [ ] In `operation.rs` tests: `Verb::from_alias("search") == Some(Verb::Search)`; `Verb::from_alias("ls") == Some(Verb::List)`; `Verb::from_alias("find") == Some(Verb::List)`; `is_valid_operation(Verb::Search, Noun::Tasks)` true.
- [ ] A schema test asserting the generated op list contains `search tasks` (mirror the existing kanban schema-generation test if present).
- [ ] `cargo test -p swissarmyhammer-kanban dispatch` and `cargo test -p swissarmyhammer-kanban operation` pass.

## Workflow
- Use `/tdd` — write failing tests first, then implement to pass.