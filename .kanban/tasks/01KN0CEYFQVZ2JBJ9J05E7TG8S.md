---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffc80
title: '[Medium] list_available_commands duplicates template resolution logic from scope_commands'
---
**File**: `kanban-app/src/commands.rs` — `list_available_commands` function (lines ~1272-1289)\n\n**Issue**: `list_available_commands` contains its own `{{entity.type}}` template resolution loop that duplicates the logic in `scope_commands::commands_for_scope()` and `resolve_name_template()`. The newer `list_commands_for_scope` endpoint correctly delegates to `commands_for_scope` which handles all template resolution in one place.\n\nThis means `list_available_commands` and `list_commands_for_scope` can return different names for the same command, depending on which code path handles the template.\n\n**Severity**: Medium (maintainability / correctness risk)\n**Layer**: Design/Architecture\n\n**Fix**: Migrate `list_available_commands` to use `commands_for_scope` like `list_commands_for_scope` does, or deprecate `list_available_commands` in favor of the newer endpoint."