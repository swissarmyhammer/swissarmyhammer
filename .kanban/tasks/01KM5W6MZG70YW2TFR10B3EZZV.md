---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffb180
title: '[warning] Multi-window inspector persistence hardcodes "main"'
---
## `kanban-app/src/commands.rs` — dispatch_command persistence block\n\nThe new persistence side-effect in `dispatch_command` always writes to `config.windows[\"main\"]`. The deleted `set_inspector_stack` accepted a `window_label` parameter for multi-window support. Secondary windows' inspector stacks are no longer persisted.\n\n`dispatch_command` doesn't currently receive a `window_label` parameter, so there's no way to know which window initiated the command.\n\n## Fix options\n1. Add an optional `window_label` parameter to `dispatch_command` (frontend already sends `boardPath`, could send window label too)\n2. Accept this limitation for now and document it — multi-window inspector persistence is a follow-up\n\n## Subtasks\n- [ ] Decide approach (extend dispatch_command or defer)\n- [ ] If extending: add window_label param to dispatch_command, use it for persistence\n- [ ] Verify fix works"