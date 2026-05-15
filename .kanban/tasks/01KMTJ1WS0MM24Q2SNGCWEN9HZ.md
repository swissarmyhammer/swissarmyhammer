---
assignees:
- claude-code
depends_on:
- 01KMTJ0EE012B95TJVPV5WJ19P
- 01KMTJ0RPCMGC0TYASH3861JGZ
position_column: done
position_ordinal: ffffffffffffffff9a80
title: End-to-end undo/redo integration tests in Rust
---
## What

Write comprehensive Rust-only integration tests that exercise the full undo/redo flow without any UI. These tests use `EntityContext` directly and verify both entity state AND the on-disk `undo_stack.yaml` file.

**Test scenarios in `swissarmyhammer-entity/tests/undo_redo_stack.rs` (new):**

1. **Single field edit undo/redo cycle:**
   - Create entity → update field → check can_undo → read `undo_stack.yaml` and verify entry present → undo → verify field reverted + pointer decremented in YAML → redo → verify restored + pointer incremented

2. **Multi-step undo:**
   - Create → update A → update B → update C → undo → verify C reverted → undo → verify B reverted → redo → verify B restored → read YAML and verify pointer position matches

3. **Undo after new edit clears redo:**
   - Create → update A → undo → update B → verify can_redo is false → read YAML and verify redo entries gone

4. **Transaction undo (multi-entity):**
   - Set transaction → update entity1 + entity2 → clear transaction → verify single entry in YAML → undo → verify both reverted in one operation

5. **Delete + undo:**
   - Create → delete → undo → verify entity restored → YAML shows correct state

6. **Stack capacity:**
   - Push 101 operations → verify oldest was trimmed in YAML, can still undo 100

7. **YAML round-trip / persistence:**
   - Perform some operations → drop EntityContext → create new EntityContext from same root → verify undo stack loaded from disk with correct pointer and entries

**Files to create:**
- `swissarmyhammer-entity/tests/undo_redo_stack.rs`

## Acceptance Criteria
- [x] All 7 test scenarios pass
- [x] Tests run with `cargo nextest run -p swissarmyhammer-entity`
- [x] No UI dependency — pure Rust tests
- [x] Tests exercise actual EntityContext, not mocks
- [x] Tests read and verify `undo_stack.yaml` on disk as a test artifact

## Tests
- [x] `cargo nextest run -p swissarmyhammer-entity --test undo_redo_stack` — all pass