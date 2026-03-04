---
position_column: done
position_ordinal: k0
title: Extract shared test_fields_context() helper to avoid duplication
---
**Review finding: Nit**

The exact same `test_fields_context()` helper function exists in both `swissarmyhammer-entity/src/context.rs` (unit tests) and `swissarmyhammer-entity/tests/undo_redo.rs` (integration tests).

- [ ] Create a `swissarmyhammer-entity/src/test_utils.rs` module behind `#[cfg(test)]`
- [ ] Move the shared helper there
- [ ] Update both test files to use it
- [ ] Run full test suite