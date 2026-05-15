---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff9080
title: 'NIT: Verbose crate::watcher:: prefix used 30+ times in flush_and_emit_for_handle'
---
kanban-app/src/commands.rs:1431-1597\n\nThe function uses the fully-qualified path `crate::watcher::WatchEvent` over 30 times instead of importing it at the top of the function or file. This makes the already long function harder to read.\n\nSuggestion: Add `use crate::watcher::{WatchEvent, BoardWatchEvent};` at the file level (other imports from crate::watcher are already used in the file) or at the function level.",
<parameter name="tags">["review-finding"] #review-finding