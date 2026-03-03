---
title: Remove vestigial columns/swimlanes fields from Board struct
position:
  column: todo
  ordinal: d3
---
In `types/board.rs`, the `Board` struct still has `columns: Vec<Column>` and `swimlanes: Vec<Swimlane>` fields annotated with `#[serde(default, skip_serializing)]`. These are vestigial from the typed storage model and can be removed once the deprecated path is gone.

- [ ] Remove `columns` and `swimlanes` fields from `Board` struct
- [ ] Remove any code that populates these fields
- [ ] Verify tests still pass