---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvn6rsf88pg05z5v8kwc9bm0
  text: |-
    Implemented. Added `req_alias(op, primary, alt)` helper next to `req` in crates/swissarmyhammer-kanban/src/dispatch.rs (returns `get_string(primary).or_else(get_string(alt))`, errors `missing required field: id (or column)`). Wired it into the `(Get|Update|Delete, Column)` arms — `add column` left on `req(op,"id")` since `column` is not a natural alias when creating. `move task`/`list tasks` untouched (their `column` is a destination/filter). Parser confirmed: explicit `op` key takes Strategy-1 precedence, so `{"op":"get column","column":"todo"}` never mis-infers `move task`.

    TDD: wrote 4 failing tests first (saw 4 FAILED for missing-field reason), then made green. Tests in dispatch test module via real parse_input→execute_operation: alias==id equality for get; missing-field error names both id and column; update/delete via alias.

    Evidence: `cargo test -p swissarmyhammer-kanban` lib 1299 passed / 0 failed (full suite all green); `cargo clippy -p swissarmyhammer-kanban --all-targets` exit 0 no warnings; `cargo fmt --check` clean. double-check agent: PASS, no findings. Moving to review.
  timestamp: 2026-06-21T13:43:53.064110+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffcc80
title: 'kanban: accept `column` as an alias for `id` on `get column`'
---
## Report (from monitoring agent)

`kanban get column {column:"done"}` → `parse error: missing required field: id`. The `get column` op requires the field name `id`, but `column` is the natural name for it, so agents hit a parse-retry (recovered, but avoidable friction).

Request: allow both `id` and `column` as parameter-name aliases on this operation.

## Verified scope

The kanban MCP tool dispatches through `KanbanOperation` in `crates/swissarmyhammer-kanban/src/dispatch.rs`. The exact site is the `(Verb::Get, Noun::Column)` arm:

```rust
// dispatch.rs:273-276
(Verb::Get, Noun::Column) => {
    let id = req(op, "id")?;                       // <- only accepts "id"
    processor.process(&GetColumn::new(id), ctx).await
}
```

`req(op, "id")` (dispatch.rs:30-33) is what emits `missing required field: id` when the key is absent. The field-alias idiom already exists in this same file — `AddTag` does:
```rust
let name = op.get_string("name").or_else(|| op.get_string("id")) ...
```

## Fix

Accept `column` as an alias for `id` in the `get column` arm. Either inline:
```rust
let id = op.get_string("id")
    .or_else(|| op.get_string("column"))
    .ok_or_else(|| KanbanError::parse("missing required field: id (or column)"))?;
```
…or add a small `req_alias(op, primary, alt)` helper next to `req` and route through it (cleaner if extended to siblings — see below). Update the error message so a genuinely-missing field tells the caller both accepted names.

Aliasing is unambiguous here: for a column entity, `column` IS the id. (Note it does NOT conflict with other ops where `column` means something else — `list tasks` / `move task` use `column` as a destination/filter; those are untouched.)

## Optional consistency follow-up (same friction on sibling column ops)

`update column` (dispatch.rs ~277) and `delete column` also take `id` via `req(op, "id")`. For the same reason an agent would naturally pass `column`. If a `req_alias` helper is added, applying it to those two arms too is a cheap consistency win. Scope this primarily to `get column` per the request; do the siblings only if the helper makes it trivial.

## Tests (in `dispatch.rs` tests — real parse→dispatch path)

The test module already uses `parse_input(json!({...}))` + `execute_operation`. Add:
- `get column {"column": "todo"}` succeeds and returns the same result as `{"id": "todo"}`.
- `get column {"id": "todo"}` still works (no regression).
- `get column {}` (neither) → parse error whose message names both `id` and `column`.
- (If siblings done) analogous alias test for `update column` / `delete column`.

## Key files (all in `swissarmyhammer-search` worktree, branch `search`)
- `crates/swissarmyhammer-kanban/src/dispatch.rs` (the `(Verb::Get, Noun::Column)` arm ~273; `req` helper ~30; AddTag alias precedent ~718)
- `crates/swissarmyhammer-kanban/src/column/get.rs` (the `GetColumn` typed command — no change needed; dispatch builds it from the resolved id)