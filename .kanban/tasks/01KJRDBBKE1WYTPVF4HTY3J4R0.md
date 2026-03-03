---
position_column: done
position_ordinal: g4
title: MoveTask uses Ordinal::first() as sentinel, conflating explicit a0 with auto-calculate
---
**Done.** Replaced Position-based sentinel with explicit Option<String> ordinal field.\n\n- [x] Changed MoveTask to use flat column/swimlane/ordinal fields instead of Position\n- [x] ordinal: Option<String> where None = auto-calculate (append at end)\n- [x] Updated MCP dispatch to pass swimlane and ordinal from input\n- [x] Updated Tauri app command\n- [x] Updated CompleteTask test\n- [x] 216 tests pass, clippy clean across kanban + tools + app