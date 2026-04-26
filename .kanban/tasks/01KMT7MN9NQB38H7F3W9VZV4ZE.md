---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff8380
title: Add tests for CommandContext::with_window_label / with_ui_state builders
---
context.rs:64-72\n\nBuilder methods on CommandContext:\n- `with_window_label(label)` — sets window_label field\n- `with_ui_state(ui_state)` — sets ui_state field\n\nTest cases:\n1. with_window_label sets the field, verify via ctx.window_label\n2. with_ui_state sets the field, verify via ctx.ui_state\n3. Chaining both builders in one expression