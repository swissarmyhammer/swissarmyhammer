---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8c80
title: 'Hide stray "Switch View" (`ui.view.set`) from command palette — missing `visible: false`'
---
## What

A bare "Switch View" entry shows up in the command palette and does nothing when invoked. It is distinct from the per-view "Switch to <ViewName>" entries (which are emitted dynamically as `view.switch:{id}` and work correctly). The stray entry is the template command `ui.view.set` in `swissarmyhammer-commands/builtin/commands/ui.yaml:30-34`:

```yaml
- id: ui.view.set
  name: Switch View
  params:
    - name: view_id
      from: args
```

**Why it silently no-ops**: The `view_id` param comes `from: args` — meaning a caller must supply it in the invocation's `args` map. The command palette has no UI for collecting arbitrary args, so when the user clicks the palette entry, `args` is empty, no `view_id` is passed, and `SetActiveViewCmd::execute` in `swissarmyhammer-kanban/src/commands/ui_commands.rs` either errors silently or no-ops.

**How view switching is actually meant to work** (traced end-to-end and confirmed working):
- `swissarmyhammer-kanban/src/scope_commands.rs:238-259` — `emit_view_switch` emits one `view.switch:{id}` ResolvedCommand per known view, with the name `Switch to <ViewName>`. These are the entries the user sees and clicks.
- `kanban-app/src/commands.rs:1314` — the dispatcher rewrites `view.switch:{id}` → `ui.view.set` with `view_id` = `{id}`, synthesizing the arg the palette cannot supply.
- `swissarmyhammer-kanban/src/commands/ui_commands.rs::SetActiveViewCmd` — actually performs the switch.

The template `ui.view.set` command is machinery glue, not a user-facing palette entry. Its peers in ui.yaml are all correctly marked `visible: false` for the same reason:
- `ui.palette.close` (line 28): `visible: false`
- `ui.perspective.set` (line 38): `visible: false` — direct analog of `ui.view.set` for perspectives
- `ui.mode.set` (line 48): `visible: false`
- `ui.setFocus` (line 56): `visible: false`

`ui.view.set` is the lone odd-one-out. The fix is a one-line YAML change: add `visible: false` alongside the existing fields at `ui.yaml:30-34`.

**What visible controls** (verified via `swissarmyhammer-commands/src/types.rs:75-76` and `swissarmyhammer-kanban/src/scope_commands.rs:672, 777, 826`): `visible` defaults to `true` (`default_true()` in types.rs:90-92). When `false`, `commands_for_scope`'s scoped/cross-cutting/global emitters all skip the command (`if !cmd_def.visible { continue; }`). The command remains fully dispatchable — only the palette / context-menu surface hides it.

## Approach

1. Edit `swissarmyhammer-commands/builtin/commands/ui.yaml`. Under `- id: ui.view.set`, add `visible: false` (preserve existing `name`, `params`). Match the indentation and field ordering used by the sibling `ui.perspective.set` entry at lines 36-41.

2. No code changes anywhere else. The dispatch path already works — the stray entry is purely a YAML visibility issue.

3. Add a hygiene test so the same foot-gun cannot reappear on any future `ui.*` command that takes args.

## Acceptance Criteria

- [ ] The command palette no longer lists a "Switch View" entry.
- [ ] The palette still lists `Switch to <ViewName>` for each registered view (`view.switch:{id}` emission unchanged — see `scope_commands.rs:238-259`).
- [ ] Left-clicking a view button in the left-nav sidebar still switches the active view (dispatch `view.switch:{id}` → rewrite to `ui.view.set` → `SetActiveViewCmd`).
- [ ] Keyboard shortcuts / programmatic dispatch of `ui.view.set` with `args: { view_id: ... }` still works (the `visible` flag does not affect dispatch).

## Tests

- [ ] New Rust test `ui_yaml_arg_only_commands_are_hidden_from_palette` in `swissarmyhammer-commands/src/registry.rs` tests module (mirrors `keymap_commands_are_visible_in_palette` at line 580-598 and the perspective hygiene pattern at line 634-648):
  1. Load `builtin/commands/ui.yaml` via `CommandsRegistry::from_yaml_sources(&[("ui", include_str!("../builtin/commands/ui.yaml"))])`.
  2. Assert every listed command with `visible == false` (all should be present and hidden):
     - `ui.view.set`, `ui.perspective.set`, `ui.mode.set`, `ui.palette.close`, `ui.setFocus`
  3. Assert every listed command with `visible == true`:
     - `ui.inspect`, `ui.inspector.close`, `ui.inspector.close_all`, `ui.palette.open`, `ui.perspective.startRename`
  4. Include a failure message that points the reader at this task: `"ui.view.set requires a view_id arg — the palette cannot provide it, so the command must be visible: false. See ui.yaml."`
- [ ] Existing tests still pass:
  - `swissarmyhammer-commands/src/registry.rs` tests module
  - `swissarmyhammer-kanban/src/commands/ui_commands.rs` tests (the two `ui.view.set` dispatch tests at lines 353-412)
  - Any integration test in `swissarmyhammer-kanban/tests/command_dispatch_integration.rs` that exercises `view.switch:*` → `ui.view.set` rewriting
- [ ] Run: `cargo test -p swissarmyhammer-commands` and `cargo test -p swissarmyhammer-kanban ui_commands` — all passing.

## Workflow

- Use `/tdd` — write the hygiene test first; it should fail because `ui.view.set.visible == true`. Add `visible: false` to the YAML entry to make it pass.
- Do not touch any Rust code, any other YAML file, or any of the `view.switch` / `SetActiveViewCmd` logic. The fix is a one-line YAML addition plus one new test. #bug #commands #ux