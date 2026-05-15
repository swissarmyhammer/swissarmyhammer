---
assignees:
- claude-code
depends_on:
- 01KPXY7Q6980X2R5DVNVCY4SZK
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9280
title: Move ui.view.set / ui.perspective.set out of generic ui.yaml into kanban domain
---
## What

After `swissarmyhammer-commands` is made fully generic (see parent task **01KPXY7Q6980X2R5DVNVCY4SZK**), `swissarmyhammer-commands/builtin/commands/ui.yaml` will still contain two commands that know about kanban's View and Perspective concepts:

- `ui.view.set` — takes a `view_id` arg, dispatches to `SetActiveViewCmd`. "View" is a kanban concept (defined in `swissarmyhammer-views`, the UI crate has no generic notion of Views).
- `ui.perspective.set` — takes a `perspective_id` arg, dispatches to `SetActivePerspectiveCmd`. "Perspective" is a kanban concept (defined in `swissarmyhammer-perspectives`).

The `ui.` prefix is misleading — these are not generic UI mechanics like `ui.palette.open` or `ui.inspector.close`; they are kanban navigation commands masquerading as UI commands. They must move into the kanban builtin set and shed the `ui.` prefix so `ui.yaml` ends up exclusively genuinely-generic UI mechanics.

### Proposed IDs and destinations

- `ui.view.set` → **`view.set`**, new file `swissarmyhammer-kanban/builtin/commands/view.yaml`. This is a fresh namespace — no existing `view.*` builtin commands today; the `view.switch:{id}` dynamic palette entries rewrite through the prefix matcher and will now rewrite to `view.set`.
- `ui.perspective.set` → **`perspective.set`**, appended to the existing `swissarmyhammer-kanban/builtin/commands/perspective.yaml` (created by the parent task).

Renaming (not just relocating) is the correct move: the `ui.` prefix is the source of the leak, and renaming also brings these two in line with the other domain-owned commands (`perspective.save`, `perspective.load`, `perspective.goto`, etc.).

### Blast radius (from grep)

ID rename sites that need coordinated updates:

- `swissarmyhammer-commands/builtin/commands/ui.yaml` — delete the two entries (lines 32–44 as of this writing).
- `swissarmyhammer-commands/src/registry.rs::ui_yaml_arg_only_commands_are_hidden_from_palette` (lines ~601–611) — remove `ui.view.set` and `ui.perspective.set` from the hidden-ID list; their palette-visibility test moves to the kanban crate.
- `swissarmyhammer-kanban/builtin/commands/view.yaml` — **new file**, one entry for `view.set` (visible: false, one `view_id` param `from: args`, not undoable).
- `swissarmyhammer-kanban/builtin/commands/perspective.yaml` — append the `perspective.set` entry (visible: false, one `perspective_id` param `from: args`, not undoable).
- `swissarmyhammer-kanban/src/commands/mod.rs` — update the two `map.insert(...)` sites (around lines 137 and 141) to register the new IDs. Update any test in this file that asserts on the `ui.view.set` / `ui.perspective.set` string (line ~765 has a `cmds.get(\"ui.view.set\")` assertion).
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — update the two `CommandContext::new(\"ui.view.set\", ...)` test sites (lines ~372, ~402) to the new id.
- `kanban-app/src/commands.rs::match_dynamic_prefix` (lines ~1313, ~1321) — update the rewrite targets so `view.switch:{id}` → `view.set{view_id}` and `perspective.goto:{id}` → `perspective.set{perspective_id}`.
- `kanban-app/ui/src/lib/views-context.tsx` (line 37) — update `useDispatchCommand(\"ui.view.set\")` to `useDispatchCommand(\"view.set\")`.
- `kanban-app/ui/src/lib/perspective-context.tsx` (lines ~128, 159, 227) — update all `dispatch(\"ui.perspective.set\", ...)` calls and the doc comment that names the command.
- `kanban-app/ui/src/lib/perspective-context.test.tsx` (lines ~176, 196, 409, 414, 436, 462, 491, 533) — every string literal and comment naming the old id.

### Out of scope

- Moving Rust impls (`SetActiveViewCmd`, `SetActivePerspectiveCmd`) out of `swissarmyhammer-kanban/src/commands/ui_commands.rs`. The impls stay where they are; only the YAML declaration and command id are renamed. Command registration and impl location are independent questions.
- Adding `builtin/commands/` directories to `swissarmyhammer-views` or `swissarmyhammer-perspectives`. Those crates are data/store crates without command infrastructure today; giving them one is a larger architectural change out of scope here.
- Any other residual leaks in `ui.yaml` (none currently identified — `ui.inspect`, `ui.inspector.*`, `ui.palette.*`, `ui.setFocus`, `ui.mode.set`, `window.new` are all genuinely generic).

### Dependencies

- **Depends on 01KPXY7Q6980X2R5DVNVCY4SZK** — the parent task establishes the per-crate builtin stacking infrastructure (`swissarmyhammer_kanban::builtin_yaml_sources()`, kanban's `builtin/commands/` directory, and the app-side chaining). This task reuses that infrastructure to host the two relocated definitions.

### Subtasks

- [x] Add `swissarmyhammer-kanban/builtin/commands/view.yaml` with `view.set` (fields mirror the original `ui.view.set` entry exactly).
- [x] Append `perspective.set` to `swissarmyhammer-kanban/builtin/commands/perspective.yaml`.
- [x] Delete `ui.view.set` and `ui.perspective.set` from `swissarmyhammer-commands/builtin/commands/ui.yaml`.
- [x] Rename every string reference (`ui.view.set` → `view.set`, `ui.perspective.set` → `perspective.set`) across Rust (commands/mod.rs, commands/ui_commands.rs, kanban-app/src/commands.rs), TypeScript (views-context.tsx, perspective-context.tsx, perspective-context.test.tsx), and the `ui_yaml_arg_only_commands_are_hidden_from_palette` test list.
- [x] Add a kanban-crate test that asserts both `view.set` and `perspective.set` are registered with `visible: false` and accept a single `view_id`/`perspective_id` arg respectively.

## Acceptance Criteria

- [x] `swissarmyhammer-commands/builtin/commands/ui.yaml` contains only genuinely-generic UI commands; grepping its contents for `view`, `perspective`, `board`, `task`, `column`, `tag`, `attachment` turns up nothing.
- [x] `swissarmyhammer-kanban/builtin/commands/view.yaml` exists and declares `view.set`.
- [x] `swissarmyhammer-kanban/builtin/commands/perspective.yaml` includes `perspective.set`.
- [x] The command ids `ui.view.set` and `ui.perspective.set` no longer appear anywhere in the workspace (verify via `rg 'ui\.view\.set|ui\.perspective\.set'` — zero hits outside git history/docs).
- [x] The dynamic-prefix rewriter still resolves `view.switch:{id}` to the live `view.set` command and `perspective.goto:{id}` to `perspective.set`.
- [x] Clicking a view icon in the left-nav and clicking a perspective tab both still trigger the backend `set_active_view` / `set_active_perspective` operations (no regression in the user-observable flow).
- [x] `cargo test -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app` passes.
- [x] `pnpm -C kanban-app/ui test --run` passes — every updated test references the new ids.

## Tests

- [x] Update `swissarmyhammer-commands/src/registry.rs::ui_yaml_arg_only_commands_are_hidden_from_palette` — remove `ui.view.set` / `ui.perspective.set` from the `hidden` list; asserts only remaining ui.* hidden commands. After edit, run `cargo test -p swissarmyhammer-commands ui_yaml_arg_only_commands_are_hidden_from_palette` and expect pass.
- [x] Add a test in `swissarmyhammer-kanban/tests/builtin_commands.rs` (the file added by the parent task) named `view_set_and_perspective_set_registered_hidden` — composes the kanban builtin source, asserts `get(\"view.set\")` and `get(\"perspective.set\")` both return `Some` with `visible == false` and a single expected-named param from `args`.
- [x] Update the four existing `perspective-context.test.tsx` expectations at lines 176, 196, 409–414, 436, 462, 491, 533 — swap the string literal from `\"ui.perspective.set\"` to `\"perspective.set\"`. Run `pnpm -C kanban-app/ui test --run perspective-context` and expect pass.
- [x] Add an integration assertion in `kanban-app/src/commands.rs` tests (or wherever `match_dynamic_prefix` is covered) verifying the rewrite produces `view.set` / `perspective.set`. If no existing test covers the rewriter, add one to the same file.
- [x] Command to run: `cargo test -p swissarmyhammer-commands -p swissarmyhammer-kanban -p kanban-app && pnpm -C kanban-app/ui test --run` — every test passes.

## Workflow

- Use `/tdd` — rewrite the commands-crate `ui_yaml_arg_only_commands_are_hidden_from_palette` list first (it'll fail because the YAML still contains the entries), then make it pass by removing the entries from `ui.yaml`. Add the kanban-side view/perspective YAML and test, watch those fail with "command not registered" until the registration-map rename is done. Work the frontend dispatch sites last.
- Stage the two renames together (`ui.view.set` → `view.set` and `ui.perspective.set` → `perspective.set`) — splitting them produces a half-renamed tree where some palette entries and some dispatch rewrites reference different ids. #commands #organization #refactor

## Review Findings (2026-04-23 16:17)

### Nits
- [x] `swissarmyhammer-commands/builtin/commands/ui.yaml:32-33` — `ui.perspective.startRename` survives in `ui.yaml` and contains the word `perspective`, so the strict reading of the acceptance criterion ("grepping its contents for `view`, `perspective`, `board`, `task`, `column`, `tag`, `attachment` turns up nothing") is not fully satisfied. Everything else the task called out is complete and correct. `ui.perspective.startRename` is a legitimate UI-layer command — the frontend intercepts it to toggle inline-rename mode and the backend impl is a no-op — so it is not the same kind of "kanban-domain leak" that the task targeted (`.set` commands that mutate domain state). The task title, subtasks, and blast radius all reference `.set` exclusively, so this is arguably out of scope. Either (a) accept that the acceptance-criterion grep overreached relative to the actual task boundary and check this off as "won't-fix in this task", or (b) follow up with a separate card to rename `ui.perspective.startRename` → `perspective.startRename` (plus the `register_ui()` → new `register_perspective()` move and the frontend `useDispatchCommand` call). No code change required in this task either way.

**Resolution (2026-04-23 follow-up)**: Addressed by filing a dedicated follow-up task — **01KPY3ETHT59CSK19JAJCRP420** "Audit ui.perspective.startRename for kanban-domain leak" — which captures the three design options (keep-generic-rename, move-to-kanban, keep-as-is-with-rationale), the full list of usage sites, and acceptance criteria for whichever path is chosen. This task's scope was deliberately limited to `.set` commands that mutate domain state; `ui.perspective.startRename` is a frontend-intercepted UI primitive whose ownership belongs to its own design conversation rather than widening this task unilaterally.