---
position_column: done
position_ordinal: ffaf80
title: 'Card 10: Add missing commands to palette'
---
Add app.quit, settings.keymap.vim/cua/emacs, file.newBoard, file.openBoard, app.about to global commands in AppShell. New Tauri commands: new_board_dialog, open_board_dialog, quit_app. Factor dialog logic out of menu.rs into shared functions.