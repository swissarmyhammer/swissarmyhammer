---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Add Archive/Unarchive verbs and Archived noun to operation types
---
## What

Add `Archive` and `Unarchive` to the `Verb` enum, and `Archived` to the `Noun` enum in `swissarmyhammer-kanban/src/types/operation.rs`.

### Changes
- `Verb::Archive` — `as_str()` returns `"archive"`, `from_alias()` matches `"archive"`
- `Verb::Unarchive` — `as_str()` returns `"unarchive"`, `from_alias()` matches `"unarchive"` and `"restore"`
- `Noun::Archived` — `as_str()` returns `"archived"`, `from_alias()` matches `"archived"`
- `Display` impls for all three

### Files
- `swissarmyhammer-kanban/src/types/operation.rs`

## Acceptance Criteria
- [ ] `Verb::from_alias("archive")` returns `Some(Verb::Archive)`
- [ ] `Verb::from_alias("unarchive")` returns `Some(Verb::Unarchive)`
- [ ] `Verb::from_alias("restore")` returns `Some(Verb::Unarchive)`
- [ ] `Noun::from_alias("archived")` returns `Some(Noun::Archived)`
- [ ] Existing dispatch match arms still compile (may need `_ =>` or explicit new arms)

## Tests
- [ ] `cargo test -p swissarmyhammer-kanban`
- [ ] Verify parse layer handles `"archive task"` string (check `parse/mod.rs`)