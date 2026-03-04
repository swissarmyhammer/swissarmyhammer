---
position_column: done
position_ordinal: j7
title: restore_entity_files should error when trash data file is missing
---
**Review finding: W5 (warning)**

`swissarmyhammer-entity/src/io.rs` — `restore_entity_files()`

Both data file and changelog rename operations silently ignore NotFound. This means restore_from_trash returns Ok(()) even when there's nothing to restore. The subsequent read_entity call then fails with a confusing "not found" error.

- [ ] Return error when the primary data file is not found in trash (changelog missing is tolerable)
- [ ] Add RestoreFromTrashFailed error variant
- [ ] Update test to verify clear error message
- [ ] Verify fix