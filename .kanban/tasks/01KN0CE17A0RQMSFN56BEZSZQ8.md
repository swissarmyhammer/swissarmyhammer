---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff680
title: '[Medium] Duplicate clipboard command registration in register_commands()'
---
**File**: `swissarmyhammer-kanban/src/commands/mod.rs` lines 52-63 and 93-104\n\n**Issue**: `entity.copy`, `entity.cut`, and `entity.paste` are inserted into the HashMap twice — once at lines 52-63 and again at lines 93-104. Because `HashMap::insert` overwrites, the second set silently replaces the first. The values are identical (type aliases), so there is no behavioral bug, but this is dead code clutter that obscures intent.\n\n**Severity**: Medium (code quality)\n**Layer**: Functionality/Correctness\n\n**Fix**: Remove the duplicate block at lines 93-104 (the second // Clipboard commands section)."