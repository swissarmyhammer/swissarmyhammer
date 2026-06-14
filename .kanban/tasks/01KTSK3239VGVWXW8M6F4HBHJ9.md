---
assignees:
- claude-code
position_column: todo
position_ordinal: e880
title: 'views: `set view` is full-replace — partial updates silently drop icon/card_fields/commands'
---
## What
Discovered while fixing 01KTCRY5W2BP7TYTHV4JB9CH8K (all views showed the LayoutGrid icon).

`SetView` (`crates/swissarmyhammer-views/src/operations.rs`) defaults every field (`#[serde(default)]`), and `handle_set_view` (`crates/swissarmyhammer-views/src/server.rs`) builds a complete `ViewDef` from the request with `commands: Vec::new()`. The write is full-replace: a caller who intends a partial update (e.g. `{op: \"set view\", id, name, kind}`) silently wipes `icon`, `card_fields`, and `commands` from the on-disk view file.

This is exactly how the degenerate `{id, name: '', kind: unknown}` files got written over the builtin grid views on this board (committed in 5d69e2eeb). The empty-name case is now rejected (`ViewDef::validate` + `ViewsContext::write_view`), and degenerate files no longer shadow builtins — but a *named* partial `set view` still destroys the unspecified fields.

## Acceptance Criteria
- [ ] Decide and implement the contract: either read-modify-write merge semantics for omitted optional fields, or explicit full-replace documented in the op schema (and the tool description warns about it).
- [ ] A `set view` that omits `icon`/`card_fields` on an existing view no longer silently strips them (or the replace contract is explicit and tested).
- [ ] Regression test at the views server layer. #bug