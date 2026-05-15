---
assignees:
- claude-code
position_column: done
position_ordinal: fffffff380
title: '[warning] Double file read in PreToolUseFileTracker'
---
avp-common/src/chain/links/file_tracker.rs:108-114\n\n`hash_files(&paths)` reads each file to compute SHA-256, then lines 111-113 read the same files again via `std::fs::read(path)` to stash content. For large files this doubles I/O.\n\nRead once, hash the bytes, stash the bytes. Could be a follow-up since this isn't a hot path.\n\n**Verify**: check that `hash_file` and content stash share a single read. #review-finding