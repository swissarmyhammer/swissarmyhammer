---
assignees:
- claude-code
depends_on:
- 01KNJ6HNYNBT3FAGDBYDEGNXPY
position_column: todo
position_ordinal: '9280'
title: Update Rust backend to handle `field:` monikers in scope chain
---
## What

Update backend Rust code that processes scope chain monikers to handle the new `field:type:id.field` format. Two specific locations need changes.

### 1. `scope_commands.rs:198-203` — Remove dot-stripping workaround

Currently strips `.field` suffix from entity IDs as a workaround for field monikers polluting the scope chain:

```rust
let base_id = entity_id.split('.').next().unwrap_or(entity_id);
let entity_moniker = format!("{entity_type}:{base_id}");
```

With `field:` prefix, field monikers arrive as `("field", "task:abc.title")` from `parse_moniker`. They won't match any entity type in `get_entity()` lookups, so the dot-stripping is no longer needed for correctness. However, the workaround should be **removed** to avoid masking future bugs, and replaced with a simple skip for `entity_type == "field"`.

### 2. `ui_commands.rs:17-25` — `first_inspectable` skips `field:` monikers naturally

`INSPECTABLE_TYPES` is `["task", "tag", "column", "board", "actor"]`. Since `parse_moniker("field:task:abc.title")` returns `("field", ...)`, and `"field"` is not in `INSPECTABLE_TYPES`, field monikers are already skipped. **No change needed here** — just verify with a test.

### 3. `commands/mod.rs` — `available()` checks on commands like `UntagTaskCmd`

`UntagTaskCmd::available` checks `ctx.has_in_scope("tag") && ctx.has_in_scope("task")`. With field monikers prefixed `field:`, `has_in_scope("task")` only matches actual `task:id` monikers — the field row `field:task:id.body` no longer matches. This is correct behavior. **No change needed** — verify the fix works end-to-end.

### Files to modify

1. `swissarmyhammer-kanban/src/scope_commands.rs:198-203` — Remove dot-stripping, add `field:` skip

### Files to verify (no changes expected)

2. `swissarmyhammer-kanban/src/commands/ui_commands.rs:17-25` — Confirm `first_inspectable` naturally skips `field:` monikers
3. `swissarmyhammer-kanban/src/commands/task_commands.rs:263-264` — `UntagTaskCmd::available` now correct
4. `swissarmyhammer-kanban/src/commands/clipboard_commands.rs:74-75` — `CutCmd::available` now correct

## Acceptance Criteria

- [ ] `scope_commands.rs` no longer strips `.field` suffixes from entity IDs
- [ ] `scope_commands.rs` skips `field:` monikers when building command lists (they're not entities)
- [ ] `first_inspectable` returns `None` for `field:task:abc.title` (already works, add test)
- [ ] `UntagTaskCmd` resolves correct task ID from scope chain `["tag:bug", "field:task:abc.tags_field", "task:abc", "column:todo"]`
- [ ] `CutCmd` resolves correct tag and task IDs from the same scope chain

## Tests

- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — Add test: scope chain with `field:task:abc.title` does not produce commands for entity `"abc.title"`
- [ ] `swissarmyhammer-kanban/src/commands/ui_commands.rs` — Add test: `first_inspectable` skips `field:` monikers
- [ ] `swissarmyhammer-kanban/src/commands/mod.rs` — Add test: `UntagTaskCmd::available` true with `["tag:bug", "field:task:abc.body", "task:abc"]`, resolves correct IDs
- [ ] Run `cargo test -p swissarmyhammer-kanban` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #field-moniker-fix