---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffec80
title: Route cancel_drag_session through dispatch_command
---
Same pattern as the drag.start card just completed.\n\n## Changes\n- Add `drag.cancel` to YAML command definitions\n- Add `DragCancelCmd` impl in `swissarmyhammer-kanban/src/commands/drag_commands.rs`\n- Register in `mod.rs`\n- Add post-execution side effect in `dispatch_command_internal` to emit `drag-session-cancelled`\n- Update frontend to use `invoke(\"dispatch_command\", { cmd: \"drag.cancel\" })`\n- Remove `cancel_drag_session` Tauri command\n\n### Tests\n- drag_cancel_cmd_clears_session\n- drag_cancel_cmd_no_session_returns_null