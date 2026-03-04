---
position_column: done
position_ordinal: j8
title: Add Eq and Hash derives to Editor, Display, SortKind enums
---
**Review finding: Nit (fields crate)**

`swissarmyhammer-fields/src/types.rs`

`Editor`, `Display`, and `SortKind` derive `PartialEq` but not `Eq` or `Hash`. These are simple enums with no floating-point members so `Eq` is trivially correct. Adding them allows use as HashMap keys.

- [ ] Add `Eq, Hash` to derive list for Editor, Display, SortKind
- [ ] Run full test suite