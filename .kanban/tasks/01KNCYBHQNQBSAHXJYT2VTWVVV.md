---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffc680
title: 'NIT: load_changed_files in file_tracker.rs is dead code (superseded by load_changed_files_as_strings)'
---
avp-common/src/chain/links/file_tracker.rs:297-311 and avp-common/src/chain/links/mod.rs:7\n\nThe function `load_changed_files` returns `Vec<PathBuf>` and is still exported from the chain module. However, the validator executor link uses `load_changed_files_as_strings` (which returns `Vec<String>`) and `load_changed_files_for_stop` (a method on the link itself). The PathBuf variant appears to have no callers outside tests.\n\nSuggestion: Verify there are no external callers via `cargo doc --document-private-items` or grep, and remove if dead." #review-finding