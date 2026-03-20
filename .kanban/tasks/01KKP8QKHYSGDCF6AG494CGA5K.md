---
position_column: done
position_ordinal: bc80
title: '[Medium] Duplicated search-index sync logic across state.rs and commands.rs'
---
The exact same pattern for converting WatchEvent variants into Entity objects and calling `idx.update(entity)` / `idx.remove(id)` is copy-pasted in two places:\n\n1. `state.rs` `start_watcher` (lines 111-143) — sync on external file changes\n2. `commands.rs` `dispatch_command` (lines 790-839) — sync after command execution\n\nBoth reconstruct an Entity from the event fields with the same loop. This should be extracted into a shared helper, e.g. `fn sync_search_index(idx: &mut EntitySearchIndex, evt: &WatchEvent)`. This reduces the risk of the two paths diverging when the WatchEvent enum gains new variants.\n\nSeverity: Medium (maintainability)" #review-finding