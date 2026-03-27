---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffea80
title: '[Bug/High] nav.last vim binding "G" will never fire -- should be "Shift+G"'
---
**File:** `kanban-app/ui/src/components/app-shell.tsx:372`\n\n**What:** The `nav.last` command registers `keys: { vim: \"G\", cua: \"End\" }`. However, `normalizeKeyEvent` always produces `\"Shift+G\"` when the user presses Shift+G (uppercase G), because it detects `shiftKey: true` with an uppercase letter and prepends `\"Shift+\"`. The binding `\"G\"` without the `Shift+` prefix will never match any keyboard event.\n\n**Evidence:** `board.lastCard` in board-view.tsx correctly uses `vim: \"Shift+G\"` (line 228). The nav.last command should do the same.\n\n**Fix:** Change `vim: \"G\"` to `vim: \"Shift+G\"` in the nav.last command definition.\n\n**Severity:** High -- the vim binding for \"go to last\" is completely broken in the global scope." #review-finding