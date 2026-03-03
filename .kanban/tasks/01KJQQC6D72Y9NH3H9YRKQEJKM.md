---
position_column: done
position_ordinal: d8
title: Entity type missing standard trait impls (PartialEq, Default, Serialize)
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/entity.rs` lines 16-21

The `Entity` struct only derives `Debug` and `Clone`. Per Rust review guidelines, new public types must implement all applicable traits due to orphan rules. Missing traits:

- `PartialEq` / `Eq` -- needed for test assertions and comparison logic
- `Default` -- reasonable since fields is a HashMap (naturally empty)
- `Serialize` / `Deserialize` -- the struct has public fields that are all serde-compatible; this would enable consumers to serialize Entity values directly

`PartialEq` is the most immediately useful -- currently tests compare field-by-field which is verbose and fragile.

**Suggestion:** Add `#[derive(Debug, Clone, PartialEq, Default)]` at minimum. Consider `Serialize`/`Deserialize` behind a feature flag.

- [ ] Add `PartialEq` and `Default` derives to `Entity`
- [ ] Consider `Eq` since all fields support it
- [ ] Verify existing tests still pass #warning