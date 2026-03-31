---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffa580
title: 'Bug: keymap commands hidden from command palette by visible: false'
---
## What

The three keymap commands (`settings.keymap.vim`, `settings.keymap.cua`, `settings.keymap.emacs`) are defined in `swissarmyhammer-commands/builtin/commands/settings.yaml` with `visible: false`. This flag causes `commands_for_scope()` in `swissarmyhammer-kanban/src/scope_commands.rs:253` to skip them entirely, so they never appear in the command palette.

The commands have Rust impls registered in `swissarmyhammer-kanban/src/commands/mod.rs:170-180` and work correctly when invoked — they just can't be discovered via the palette.

### Fix

Remove `visible: false` from all three keymap commands in `swissarmyhammer-commands/builtin/commands/settings.yaml` (lines 3, 12, 22). The `visible` field defaults to `true`, so simply deleting those lines is sufficient.

### Files to modify

- **`swissarmyhammer-commands/builtin/commands/settings.yaml`** — remove `visible: false` from the three `settings.keymap.*` entries

## Acceptance Criteria

- [ ] `settings.keymap.vim`, `settings.keymap.cua`, `settings.keymap.emacs` appear in the command palette when opened
- [ ] Selecting a keymap command from the palette switches the keymap mode
- [ ] The commands still appear in the App > Settings menu with radio group behavior
- [ ] No other commands change visibility

## Tests

- [ ] Existing test `set_keymap_mode_executes` in `swissarmyhammer-kanban/src/commands/mod.rs` still passes: `cargo nextest run set_keymap_mode`
- [ ] Existing test `keymap_mode_change` in `swissarmyhammer-kanban/tests/command_dispatch_integration.rs` still passes: `cargo nextest run keymap_mode_change`
- [ ] Add a unit test in `swissarmyhammer-commands/src/registry.rs` (or existing test module) that loads the builtin registry and asserts `settings.keymap.vim`, `settings.keymap.cua`, `settings.keymap.emacs` all have `visible == true`
- [ ] `cargo nextest run` passes