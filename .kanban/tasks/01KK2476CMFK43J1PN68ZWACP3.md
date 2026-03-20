---
assignees: '@clayu'
position_column: done
position_ordinal: ffffe680
title: 'Bug: Vim : keybinding doesn''t trigger command palette'
---
: is bound to app.command in vim keybindings. createKeyHandler skips single-char keys when focus is in editable context (INPUT, TEXTAREA, .cm-editor, contenteditable). Should work when board itself has focus.\n\nLogging added to keybindings.ts — console.debug shows: mode, normalized key, target element, skip reason, match result. Check browser console with `:` key to see if it's being skipped and why.\n\nKey files: keybindings.ts (createKeyHandler lines 157-217)