---
position_column: done
position_ordinal: df80
title: 'Card 13: Wire menu-command events through command scope'
---
Menu item clicks emit menu-command event with the command ID. Frontend listens and routes through executeCommand(id). Remove old event-specific listeners (keymap-changed, board-changed) where command callbacks handle side effects.