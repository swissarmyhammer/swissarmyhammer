---
assignees:
- claude-code
position_column: todo
position_ordinal: c880
title: '[nit] Redundant UIState check pattern in all UI commands'
---
swissarmyhammer-kanban/src/commands/ui_commands.rs:35-38, 64-67, 87-90, 110-113, etc.

Every UI command (InspectCmd, InspectorCloseCmd, PaletteOpenCmd, SetFocusCmd, SetAppModeCmd, etc.) repeats the same boilerplate:

```rust
let ui = ctx.ui_state.as_ref()
    .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;
```

This is duplicated ~8 times. UIState is always present in the production context (set by dispatch_command_internal). A helper method on CommandContext like `require_ui_state()` would reduce 3 lines to 1 line per command.

Suggestion: Add `CommandContext::require_ui_state() -> Result<&UIState>` and use it in all UI commands. #review-finding