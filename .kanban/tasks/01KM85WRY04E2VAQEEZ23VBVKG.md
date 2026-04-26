---
assignees:
- claude-code
depends_on:
- 01KM85WCYYAT2XC8J2FDVRFMNT
position_column: done
position_ordinal: ffffffffffffb080
title: 'Remove AppConfig: delete struct and all references'
---
## What

By this point all state has migrated from AppConfig into UIState. AppConfig should be completely empty and unused.

### Changes
- Delete `AppConfig` struct from `kanban-app/src/state.rs`
- Delete `WindowState` struct (now in UIState)
- Delete `RecentBoard` struct (now in UIState) 
- Delete `config_file_path()` and `legacy_config_file_path()` helpers
- Delete `AppConfig::load()` and `AppConfig::save()`
- Remove `config: RwLock<AppConfig>` from AppState
- Remove all `state.config.read()` / `state.config.write()` references
- Delete the legacy JSON migration code
- Grep for any remaining `AppConfig` references and remove them

### Verification
This is a pure deletion card. If any code still references AppConfig, it means a previous migration card missed something — fix the reference, don't keep AppConfig alive.

## Acceptance Criteria
- [ ] `AppConfig` struct does not exist
- [ ] `WindowState` struct does not exist in state.rs (lives in UIState now)
- [ ] No `state.config` references anywhere
- [ ] `grep -r AppConfig kanban-app/src/` returns nothing
- [ ] App compiles and all tests pass

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] `pnpm --filter kanban-app test` passes