---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffff380
title: Stale doc comment in state.rs references "frontend manifest"
---
In `kanban-app/src/state.rs` line 262, the doc comment for `menu_items` says \"Populated when the menu is rebuilt from the frontend manifest\". The frontend manifest path is deleted -- menus are now built from the command registry on the Rust side. This comment is stale and should say \"Populated when the menu is built from the command registry\" or similar.\n\nFile: `kanban-app/src/state.rs:261-263`" #review-finding