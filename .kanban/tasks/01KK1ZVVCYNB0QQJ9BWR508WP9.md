---
position_column: done
position_ordinal: fff780
title: 'dispatch_command: load user command overrides from .kanban/commands/'
---
**File:** `swissarmyhammer-kanban-app/src/state.rs`\n\n**What:** `AppState::new()` loads builtin YAML sources but never loads user-defined command overrides from `.kanban/commands/*.yaml`. The registry supports overrides but they're never fed in.\n\n**Fix:** After opening a board, scan `.kanban/commands/` for YAML files and merge them into the registry.\n\n- [ ] Add user override loading after board open\n- [ ] Verify overrides merge correctly\n- [ ] Verify tests pass