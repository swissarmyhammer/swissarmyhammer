---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffb480
title: Window menu separator always appended even when no "window" manifest entries exist
---
kanban-app/src/menu.rs:127\n\nThe Window menu block appends a separator unconditionally inside the `if let Some(items)` branch:\n\n```rust\nif let Some(items) = menus.get(\"window\") {\n    // ... append items ...\n    window_menu.append(&PredefinedMenuItem::separator(app)?)?;  // always runs\n}\nwindow_menu.append(&PredefinedMenuItem::minimize(app, None)?)?;\n```\n\nIf the manifest has window items but the last item is already in a group that would naturally end with a separator, this produces a double separator. More importantly, if the frontend sends an empty window group, the separator is appended between nothing and Minimize.\n\nThis is the same separator-management style used in the existing File/App menus, which track `last_group` to insert separators only between groups. The window menu hardcodes the separator without that guard.\n\nSuggestion: Wrap the separator in a guard, or only append it when `items` is non-empty (it always will be here, but the pattern is inconsistent)." #review-finding