---
title: Change TagId to slug and simplify Tag struct
position:
  column: done
  ordinal: a6
---
TagId becomes a normalized lowercase string (the tag name IS the id) instead of a ULID. Remove the `name` field from Tag struct since id == name. No backward compatibility needed.

**Type changes:**
- `types/board.rs`: Remove `name: String` from Tag struct. Tag becomes `{ id, color, description }`. Update `Tag::new()` to take `(id, color)`.
- `types/ids.rs`: Update TagId doc. All call sites use `TagId::from_string(name.to_lowercase())` instead of `TagId::new()`.

**Operation changes:**
- `tag/add.rs`: Remove `name` param. Accept `id` as the tag name (normalized lowercase). Make `color` optional — default to `auto_color(id)` from Card 1.
- `tag/update.rs`: Remove `name` update option.
- `tag/{get,delete,list}.rs`: Adjust for no `name` field.
- `board/get.rs`: Adjust tag serialization — no `name` field.
- `task/tag.rs`: Adjust TagId creation for new semantics.

**MCP dispatch:**
- `swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`: Update AddTag static constructor.

**Files:** `types/{ids,board}.rs`, `tag/{add,update,delete,get,list}.rs`, `task/tag.rs`, `board/get.rs`, MCP dispatch, `tests/integration_tag_storage.rs`

- [ ] Remove name from Tag struct, update Tag::new()
- [ ] Update AddTag: remove name param, auto-color default
- [ ] Update UpdateTag: remove name option
- [ ] Update GetBoard tag serialization
- [ ] Update MCP dispatch
- [ ] Fix all tests
- [ ] cargo test passes