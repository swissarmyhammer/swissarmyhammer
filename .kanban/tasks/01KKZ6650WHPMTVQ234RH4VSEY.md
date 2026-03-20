---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffee80
title: 'Nit: board_path_str clone per event in watcher closure'
---
kanban-app/src/state.rs:176-178, kanban-app/src/commands.rs:1507-1510\n\nBoth emission sites clone `board_path_str` into every `BoardWatchEvent`. Since the string is identical for all events from one board, an `Arc<str>` would avoid the per-event allocation. However, entity events are infrequent and the string is small, so this is purely cosmetic."