---
assignees:
- assistant
depends_on:
- 01KKM7RX9ZTK4VKMB6P9NPWHWW
position_column: done
position_ordinal: z00
title: Update Tauri app config and .kanban discovery to use XDG / .sah
---
## What
Two changes in the Tauri kanban app:

### 1. App config path (`kanban-app/src/state.rs`)
- Currently: `dirs::config_dir().join(\"swissarmyhammer-kanban\").join(\"config.json\")`
- Change to: use `ManagedDirectory::<SwissarmyhammerConfig>::xdg_config()` and put app config under `$XDG_CONFIG_HOME/sah/kanban-app/config.json`
- Update `CONFIG_DIR_NAME` const from `\"swissarmyhammer-kanban\"` to derive from ManagedDirectory

### 2. .kanban → .sah/kanban (or keep .kanban?)
This needs a design decision: `.kanban` is the kanban board data directory. Options:
- **Option A**: Keep `.kanban` as-is — it's product-specific, not a config dir
- **Option B**: Move to `.sah/kanban/` — consistent but breaks existing boards

**Recommendation**: Keep `.kanban` for now. It's the board data, not app config. The `.sah` rename is about the tool's config/state dir, not board data. We can always add a migration later.

If keeping `.kanban`: just update `state.rs` config path and the `discover_board` / `resolve_kanban_path` functions' comments.

### Key files
- `kanban-app/src/state.rs` — config_file_path(), CONFIG_DIR_NAME const, discover_board comments

## Acceptance Criteria
- [ ] App config uses XDG path via ManagedDirectory
- [ ] No direct `dirs::config_dir()` call in state.rs
- [ ] Board discovery still works with `.kanban`
- [ ] Config migration: reads from old path if new path doesn't exist

## Tests
- [ ] `cargo nextest run -p kanban-app` (or whatever the package name is)
- [ ] Manual: app starts clean, config saved to new XDG path