---
position_column: done
position_ordinal: ffd880
title: 'Bug: NORMAL mode footer shows in CUA/Emacs mode'
---
ModeIndicator uses AppMode (interaction state) not KeymapMode, and renders unconditionally. Shows vim-style '-- NORMAL --' regardless of keymap.\n\nKey files: mode-indicator.tsx, app-mode-context.tsx, App.tsx:160