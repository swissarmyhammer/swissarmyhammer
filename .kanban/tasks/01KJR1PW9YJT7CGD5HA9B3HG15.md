---
position_column: done
position_ordinal: f1
title: Delete changelog entry is immediately destroyed by delete_entity_files
---
**Resolution:** Already fixed. The `delete_entity_files` function was replaced by `trash_entity_files` which moves both the data file and `.jsonl` changelog to `.trash/{type}s/` via `fs::rename`. The delete changelog entry written before the move IS preserved in the trash directory. No data loss.\n\n- [x] Decide on approach — tombstone approach already implemented via trash\n- [x] Implement the fix — `trash_entity_files` preserves the log\n- [x] Update test — doc comment already accurate\n- [x] Verify — `trash_entity_files` tests confirm move behavior