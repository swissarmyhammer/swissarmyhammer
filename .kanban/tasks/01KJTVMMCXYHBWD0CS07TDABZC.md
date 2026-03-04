---
position_column: todo
position_ordinal: c8
title: Add `Lexical` variant to SortKind
---
The spec defines two string sort modes: `lexical` (pure string comparison, the implicit default) and `alphanumeric` (smart number-aware sorting like "Item 2" before "Item 10"). The code only has `Alphanumeric`. Adding `Lexical` lets fields distinguish between the two, and makes the implicit default correct per spec.

## Enum change

In `swissarmyhammer-fields/src/types.rs`, add `Lexical` to `SortKind`:
```rust
pub enum SortKind {
    Alphanumeric,
    Lexical,        // new — pure string comparison
    OptionOrder,
    Datetime,
    Numeric,
}
```

Serializes as `"lexical"` via the existing `#[serde(rename_all = "kebab-case")]`.

## Add `effective_sort()` method

Add to `FieldDef` impl block, following the pattern of `effective_editor()` and `effective_display()`:
```rust
pub fn effective_sort(&self) -> SortKind {
    if let Some(ref s) = self.sort {
        return s.clone();
    }
    SortKind::Lexical
}
```

## Tests

- YAML round-trip for `SortKind::Lexical` (extend existing `editor_display_sort_yaml_round_trip` test)
- `effective_sort()` returns `Lexical` when `sort` is `None`
- `effective_sort()` returns explicit value when `sort` is `Some(Alphanumeric)`

## Checklist

- [ ] Add `Lexical` variant to SortKind enum
- [ ] Add `effective_sort()` method to FieldDef impl
- [ ] Add YAML round-trip test for Lexical
- [ ] Add tests for effective_sort() default and explicit behavior
- [ ] Run `cargo test -p swissarmyhammer-fields`