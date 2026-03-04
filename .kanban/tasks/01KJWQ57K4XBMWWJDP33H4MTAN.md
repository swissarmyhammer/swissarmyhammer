---
position_column: done
position_ordinal: k1
title: Document compute field ordering requirement in derive_all
---
**Review finding: Nit (fields crate)**

`swissarmyhammer-fields/src/compute.rs` — `derive_all()`

Iterates field_defs in given order. If computed field B depends on computed field A's output, the result depends on ordering. No dependency analysis or topological sort.

Today this works because computed fields read from stored fields, not other computed fields. But nothing enforces this.

- [ ] Add prominent doc comment on derive_all explaining the ordering requirement
- [ ] Consider adding a `depends_on` field to FieldType::Computed for future use
- [ ] Add a test that demonstrates two computed fields where order matters (document expected behavior)
- [ ] Run full test suite