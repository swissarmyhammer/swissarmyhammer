---
assignees:
- claude-code
depends_on:
- 01KM85WRY04E2VAQEEZ23VBVKG
position_column: done
position_ordinal: ffffffffffffb280
title: 'Remove AppState: replace with UIState as single state owner'
---
## What

By this point AppState should only contain:
- `ui_state: Arc<UIState>` — the real state owner
- `boards: RwLock<HashMap<PathBuf, Arc<BoardHandle>>>` — runtime board handles (not config)
- `commands_registry: RwLock<CommandsRegistry>` — loaded command definitions
- `command_impls: HashMap<String, Arc<dyn Command>>` — command implementations
- `shutting_down: AtomicBool` — shutdown flag

These are runtime infrastructure, not persistent state. Decide:
1. Keep AppState as a thin wrapper around UIState + runtime infra, OR
2. Move UIState to be the Tauri managed state and put runtime infra alongside it, OR
3. Rename AppState to something that reflects its new role (e.g. `AppRuntime`)

### Changes
- Remove all fields from AppState that have been migrated to UIState
- If AppState is now just `{ ui_state, boards, commands_registry, command_impls, shutting_down }`, consider whether the indirection is worth keeping
- Remove `get_ui_context` Tauri command (replaced by `get_ui_state`)
- Remove any remaining legacy Tauri commands that were kept for backwards compat
- Clean up `kanban-app/src/state.rs` — remove dead code, unused imports, unused helpers
- Final grep: no references to removed Tauri commands in frontend

### Verification
This is the final cleanup. After this card, the state architecture is:
- **UIState** (in swissarmyhammer-commands) = single owner of all persistent + transient UI state
- **BoardHandle map** (in kanban-app) = runtime Tauri concern, not state
- **CommandsRegistry** (in kanban-app) = runtime Tauri concern, not state

## Acceptance Criteria
- [ ] AppState has no persistent state — only runtime handles
- [ ] No legacy Tauri commands remain for state that UIState owns
- [ ] `get_ui_context` removed (replaced by `get_ui_state`)
- [ ] All frontend state reads come from `useUIState()` or `get_ui_state`
- [ ] App compiles and all tests pass

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes