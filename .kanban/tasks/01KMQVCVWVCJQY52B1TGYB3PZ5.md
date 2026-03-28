---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff8a80
title: '[Design/Medium] nav commands missing emacs keybindings'
---
**File:** `kanban-app/ui/src/components/app-shell.tsx:339-374`\n\n**What:** All 6 nav commands define `vim` and `cua` keys but none define `emacs` keys. Emacs users conventionally expect `C-p`/`C-n`/`C-b`/`C-f` for up/down/left/right and `M-<`/`M->` for first/last. Currently emacs mode has no nav key bindings at all.\n\n**Severity:** Medium -- emacs mode is less commonly used and this may be intentional deferral, but it should be documented if so." #review-finding