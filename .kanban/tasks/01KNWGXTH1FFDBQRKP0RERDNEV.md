---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffb680
title: 'Palette: "Rename Perspective" appears twice; make perspective rename view-aware'
---
## What

Opening the command palette shows "Rename Perspective" **twice** because two
commands share the same display name:

1. `perspective.rename` (from `swissarmyhammer-commands/builtin/commands/perspective.yaml:26-34`)
   — the backend dispatch command that requires `id` + `new_name` args.
   Cannot actually be invoked directly from the palette (no args prompt UI).
2. `ui.perspective.startRename` (from `swissarmyhammer-commands/builtin/commands/ui.yaml:43-44`)
   — the UI command added recently. Triggers inline rename on the active tab.

Also, `ui.perspective.startRename` currently reports `available: true`
unconditionally, so it shows even when there is no active perspective to
rename (e.g. on a view kind with no perspectives loaded yet). It should be
**view-aware**: only available when an active perspective exists for the
current window.

### Fix

**Hide the backend rename from the palette** (it cannot be dispatched without
args, so its palette entry is dead weight):

- `swissarmyhammer-commands/builtin/commands/perspective.yaml:26-34` — add
  `visible: false` to `perspective.rename`. It stays in the registry and
  remains dispatchable from code (the inline rename commit flow in
  `perspective-tab-bar.tsx` already dispatches it with args).

**Gate `StartRenamePerspectiveCmd::available()` on an active perspective**:

- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — `StartRenamePerspectiveCmd::available()`
  currently returns `true`. Change it to:
  1. Read `UIState::active_perspective_id(window_label)` via `ctx.ui_state` and
     `ctx.window_label_from_scope().unwrap_or("main")`.
  2. Return `true` only when the ID is non-empty.

  This makes it view-aware because `active_perspective_id` is per-window and
  switches automatically when the user switches view kind (the frontend picks
  the first perspective for the new view).

### Files to modify

- `swissarmyhammer-commands/builtin/commands/perspective.yaml` — one-line
  `visible: false` addition to `perspective.rename`
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — implement
  `available()` for `StartRenamePerspectiveCmd`
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` (tests module) — add
  unit test for the new `available()` logic
- `swissarmyhammer-commands/src/registry.rs:615-622` (`test_perspective_yaml_parses`)
  — this test currently asserts every non-`perspective.list` command is
  `visible`. After hiding `perspective.rename`, update the exclusion list to
  include it alongside `perspective.list`. (Note: this test is already failing
  pre-existing on `perspective.goto should be visible` — this card MUST also
  fix that existing assertion by adding `perspective.goto` to the exclusion
  list, since `perspective.goto` is already `visible: false` in the YAML.)

### Why hide `perspective.rename` instead of renaming `ui.perspective.startRename`?

- `perspective.rename` fundamentally cannot be invoked from the palette
  without an args prompt UI (which does not exist). Its palette entry is
  non-functional regardless of the duplicate.
- `ui.perspective.startRename` is the user-facing "enter rename mode" command
  — it deserves the friendly "Rename Perspective" name in the palette.
- This mirrors the pattern already in place for `perspective.goto` and
  `perspective.list`, both of which are `visible: false`.

## Acceptance Criteria

- [ ] Opening the command palette on a board view with an active perspective
      shows exactly one entry named "Rename Perspective" (the
      `ui.perspective.startRename` command)
- [ ] `perspective.rename` is still dispatchable programmatically from
      `perspective-tab-bar.tsx`'s `commitRename` flow (double-click rename
      and Enter-to-commit still work end-to-end)
- [ ] When no active perspective exists (fresh window, no perspective selected),
      `ui.perspective.startRename` does NOT appear in the palette
- [ ] `test_perspective_yaml_parses` passes — exclusion list updated to
      include both `perspective.goto` and `perspective.rename` alongside
      `perspective.list`

## Tests

- [ ] `swissarmyhammer-kanban/src/commands/ui_commands.rs` (tests module) —
      add `start_rename_perspective_available_requires_active_perspective`:
      1. Build a `CommandContext` with `UIState` that has no active perspective
         for the "main" window — assert `StartRenamePerspectiveCmd.available(&ctx) == false`
      2. Set `active_perspective_id = "p1"` on the window via `set_active_perspective` —
         assert `StartRenamePerspectiveCmd.available(&ctx) == true`
- [ ] `swissarmyhammer-commands/src/registry.rs` — update
      `test_perspective_yaml_parses` exclusion list to
      `["perspective.list", "perspective.goto", "perspective.rename"]`; test
      should pass after the change
- [ ] Run: `cargo test -p swissarmyhammer-kanban --lib start_rename_perspective`
- [ ] Run: `cargo test -p swissarmyhammer-commands test_perspective_yaml_parses`
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/perspective-tab-bar.test.tsx`
      (existing rename tests must still pass — the change to `perspective.rename`
      visibility does not affect the dispatch path)

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
- Existing `StartRenamePerspectiveCmd` lives in
  `swissarmyhammer-kanban/src/commands/ui_commands.rs` and was added in
  commit c250c1a3b. Start by reading it before modifying.