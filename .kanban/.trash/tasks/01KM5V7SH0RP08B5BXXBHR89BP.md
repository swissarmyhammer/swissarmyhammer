---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8780'
title: 'Cleanup: remove set_inspector_stack Tauri command'
---
## What\n\nAfter Cards 1-3, inspector stack persistence happens as a side-effect of `dispatch_command` (Card 1) and the frontend no longer calls `set_inspector_stack` directly (Card 2). The separate `set_inspector_stack` Tauri command is now dead code.\n\n### Files\n- `kanban-app/src/commands.rs` — remove `set_inspector_stack` function\n- `kanban-app/src/main.rs` — remove from `.invoke_handler(tauri::generate_handler![...])`\n\n### Approach\n1. Remove the `set_inspector_stack` function from `commands.rs`\n2. Remove it from the Tauri invoke handler registration in `main.rs`\n3. Verify no frontend code still references it (should be clean after Card 2)\n\n## Acceptance Criteria\n- [ ] `set_inspector_stack` Tauri command no longer exists\n- [ ] No frontend code references `set_inspector_stack`\n- [ ] `cargo nextest run` passes\n- [ ] `npx vitest run` passes\n\n## Tests\n- [ ] `cargo nextest run -p kanban-app` — compile success confirms no dangling references\n- [ ] `npx vitest run` — no test references the removed command\n\n## Subtasks\n- [ ] Remove `set_inspector_stack` from `commands.rs`\n- [ ] Remove from Tauri handler registration in `main.rs`\n- [ ] Grep for any remaining references\n\n## depends_on\n- Card 2: Frontend route inspect/close through dispatch_command"