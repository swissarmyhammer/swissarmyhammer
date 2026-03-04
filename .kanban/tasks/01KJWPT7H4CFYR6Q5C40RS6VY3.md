---
position_column: done
position_ordinal: i7
title: Add stale detection for non-string field undo/redo
---
**Review finding: B2 (blocker)**

`swissarmyhammer-entity/src/changelog.rs` — `apply_changes()` line ~237

For `FieldChange::Changed { old_value, new_value }`, apply_changes blindly sets new_value without checking that the current field value matches the expected state. TextDiff has natural stale detection (patch fails to apply), but Changed/Set/Removed do not.

Example: field `color` is `#00ff00`. Undo expects `#ffffff` → `#ff0000`. The undo silently sets `#ff0000` even though current is neither expected value. Silent data corruption.

## Fix approach
Add a consistency check in apply_changes for `Changed` variants: verify the current value matches the expected old_value (for forward) or new_value (for reverse). If mismatch, return an error.

Need to be careful about the direction — when applying forward changes, current should match old_value. When applying reversed changes, current should match what was new_value in the original.

- [ ] Add current-value check for FieldChange::Changed in apply_changes
- [ ] Add current-value check for FieldChange::Set (verify field doesn't already exist with different value)
- [ ] Add test: non-string stale undo returns error
- [ ] Add test: non-string stale redo returns error
- [ ] Verify fix