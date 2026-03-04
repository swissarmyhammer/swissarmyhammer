---
position_column: done
position_ordinal: j9
title: Fix effective_sort to infer from field type like effective_editor/effective_display
---
**Review finding: Nit (fields crate) — but consequential**

`swissarmyhammer-fields/src/types.rs` — `effective_sort()`

Unlike `effective_editor` and `effective_display` which infer from the field type, `effective_sort` always defaults to `Lexical`. This means Date fields sort lexically (wrong), Number fields sort lexically (wrong — "9" > "10"), and Select fields sort lexically instead of by option order.

## Fix
```rust
match &self.type_ {
    FieldType::Date => SortKind::Datetime,
    FieldType::Number { .. } => SortKind::Numeric,
    FieldType::Select { .. } | FieldType::MultiSelect { .. } => SortKind::OptionOrder,
    _ => SortKind::Lexical,
}
```

- [ ] Update effective_sort to infer from field type
- [ ] Add tests for each inferred sort kind
- [ ] Verify existing tests pass
- [ ] Run full test suite