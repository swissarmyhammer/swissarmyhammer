---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: 'Emacs Mod+f binding collision: nav.right shadows app.search'
---
**Severity: High (Functionality)**

In `kanban-app/ui/src/lib/keybindings.ts`, the emacs BINDING_TABLE maps `Mod+f` to `nav.right`. But in `app-shell.tsx`, the `app.search` command declares `keys: { emacs: "Mod+F" }`. Since the BINDING_TABLE is checked first and scope bindings overlay global bindings, the `Mod+f` -> `nav.right` in the emacs table will always win, making Find (Cmd+F / Ctrl+F) unreachable in emacs mode.

The previous emacs table had `"Mod+f": "app.search"` which was removed and replaced with `"Mod+f": "nav.right"` (via `"Ctrl+f": "nav.right"` / `"Mod+f": "nav.right"`).

**Fix:** Remove `"Mod+f": "nav.right"` from the emacs BINDING_TABLE. The emacs nav.right should only be `Ctrl+f` (on macOS) or rely on the global nav commands defined in app-shell.tsx. Alternatively, use `Ctrl+s` for search in emacs mode to avoid the conflict entirely. #review-finding