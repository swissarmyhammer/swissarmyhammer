---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffd180
title: Changelog.compact is not atomic -- data loss on crash during rewrite
---
swissarmyhammer-store/src/changelog.rs -- compact() method\n\nThe `compact` method reads all entries, filters, then calls `tokio::fs::write` to overwrite the file in-place. If the process crashes between the truncation (implicit in write) and the completion of the new content, the changelog is lost.\n\nAll other write paths in this crate use atomic temp-file-then-rename patterns (see `StoreHandle::atomic_write`). The compact operation should do the same.\n\nSuggestion: Write to a temp file (`.tmp_<ulid>`) alongside the changelog, then rename over the original. This matches the pattern used elsewhere in the crate. #review-finding