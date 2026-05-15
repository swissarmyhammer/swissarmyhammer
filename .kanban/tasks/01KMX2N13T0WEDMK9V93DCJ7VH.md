---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffcd80
title: Add menu field to YAML CommandDef
---
## What

Add `MenuPlacement` struct and optional `menu` field to `CommandDef` in `swissarmyhammer-commands/src/types.rs`. Add `menu` metadata to all YAML command files for commands that should appear in the menu bar.

### Files to modify
- `swissarmyhammer-commands/src/types.rs` — add `MenuPlacement` struct, add `menu: Option<MenuPlacement>` to `CommandDef`
- `swissarmyhammer-commands/builtin/commands/app.yaml` — add menu for quit, undo, redo
- `swissarmyhammer-commands/builtin/commands/entity.yaml` — add menu for entity.cut, entity.copy, entity.paste
- `swissarmyhammer-commands/builtin/commands/file.yaml` — add menu for file commands
- `swissarmyhammer-commands/builtin/commands/settings.yaml` — add menu for keymap modes
- `swissarmyhammer-commands/builtin/commands/ui.yaml` — add menu for find/search

### MenuPlacement struct
```rust
pub struct MenuPlacement {
    pub path: Vec<String>,  // ["Edit"] or ["File", "Export"]
    pub group: usize,
    pub order: usize,
    pub radio_group: Option<String>,
}
```

### YAML examples
```yaml
- id: entity.copy
  name: Copy
  menu:
    path: [Edit]
    group: 1
    order: 1
```

## Acceptance Criteria
- [x] `MenuPlacement` struct parses from YAML
- [x] `CommandDef` accepts optional `menu` field
- [x] All existing menu commands have `menu` metadata in YAML
- [x] `cargo nextest run -p swissarmyhammer-commands` passes
- [x] Existing YAML round-trip tests still pass

## Tests
- [x] Add YAML round-trip test for CommandDef with menu field
- [x] Verify all commands with menu metadata parse correctly