---
position_column: done
position_ordinal: d9
title: to_json silently overwrites fields named "id" or "entity_type"
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/entity.rs` lines 82-90

`to_json()` first inserts `"id"` and `"entity_type"` into the map, then iterates over `self.fields` and inserts each key. If a field happens to be named `"id"` or `"entity_type"`, it will silently overwrite the metadata values with the field value. The order of operations makes this unpredictable.

**Suggestion:** Either (a) insert the metadata *after* the fields loop (so metadata always wins), or (b) skip fields named `"id"` and `"entity_type"` from the iteration, or (c) document that these names are reserved. Option (a) is simplest and most defensive.

- [ ] Change `to_json()` to insert `id` and `entity_type` after the fields loop (or skip reserved names)
- [ ] Add a test that verifies a field named `id` does not clobber the entity ID
- [ ] Verify the fix #warning