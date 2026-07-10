---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffae80
project: builtin-commands
title: Suppress delete/archive/unarchive/inspect on fields via the applies_to capability gate
---
## Problem

`entity.delete`, `entity.archive`, `entity.unarchive`, and `app.inspect` are cross-cutting commands with **no `applies_to`** declaration, so the list-time capability gate is a no-op for them (`CommandService::applies_to_focus` returns `true` when `applies_to` is `None`). As a result they surface on the command surface (context menu / palette) for **any** focused object — including a **field**.

A `field:{type}:{id}.{name}` moniker is a *projection* of its containing entity, not an entity. When such a moniker is the explicit `ctx.target` (context menu fired over a field row, or the focused target is a field), `caption::focused_entity_type` returns `"field"` verbatim (explicit target wins, unfiltered). So a focused field shows nonsensical rows: **"Delete Field", "Archive Field", "Unarchive Field", "Inspect Field"**.

This is the same class of leak the clipboard trio already fixed: `entity.cut/copy/paste` carry `applies_to: CLIPBOARD_ENTITY_TYPES` (mirrors Rust `COPYABLE_ENTITY_TYPES`) so they never surface on types that don't support them. The four CRUD/inspect commands never got the same gate.

Decision (confirmed with the user): suppress **all four** on fields — including `inspect`. (`entity.inspect` — the Space gesture, `visible:false`, no context menu — already resolves its target server-side by skipping `field:` monikers and landing on the containing entity, so it does NOT surface "Inspect Field" and needs no change; the offending visible surface is `app.inspect`.)

## Resolution

- `CLIPBOARD_ENTITY_TYPES` renamed to `OPERABLE_ENTITY_TYPES` in `builtin/plugins/entity-commands/index.ts`, shared by the clipboard trio AND the CRUD trio (`entity.delete` / `entity.archive` / `entity.unarchive`), which all now declare `applies_to: OPERABLE_ENTITY_TYPES`.
- `app.inspect` in `builtin/plugins/app-shell-commands/commands/ui.ts` declares the same set (local `OPERABLE_ENTITY_TYPES` const, mirrored + drift-guarded). `entity.inspect` left ungated with an explanatory comment.
- `crates/swissarmyhammer-command-service/src/caption.rs` doc updated: field commands (inspect included) are now suppressed at the command surface.

## Acceptance Criteria
- [x] With a `field:` moniker as the focus (`ctx.target` = `field:task:01ABC.title`), `list command` does NOT return `entity.delete`, `entity.archive`, `entity.unarchive`, or `app.inspect`.
- [x] With a real entity as the focus (`ctx.target` = `task:01ABC`), `list command` DOES return all four (no regression).
- [x] The clipboard trio behavior is unchanged; `CLIPBOARD_ENTITY_TYPES` renamed to `OPERABLE_ENTITY_TYPES` and the existing `assert_clipboard_applies_to` drift guard still passes (delegates to the shared `assert_operable_applies_to`).
- [x] No UI/React hardcoded `if (type === "field")` branch is introduced — gating stays declarative data on the registration, interpreted by `applies_to_focus`.

## Tests
- [x] Extended the production-path e2e: `builtin_entity_commands_e2e::crud_commands_suppressed_on_a_field_offered_on_an_entity` (entity-commands bundle, CRUD trio) and `builtin_app_shell_commands_e2e::app_inspect_suppressed_on_a_field_offered_on_an_entity` (app-shell-commands bundle, app.inspect) — load the REAL bundles through the V8 isolate, drive `list command` with a `field:` target (four ids absent) and a `task:` target (present).
- [x] Added focused unit cases to `tests/list_applies_to.rs`: `crud_inspect_commands_absent_when_a_field_is_the_context_menu_target` / `crud_inspect_commands_present_when_a_task_is_focused` (delete/archive/unarchive/inspect-shaped fixtures, field-suppressed / task-offered via `applies_to_focus`).
- [x] Drift guard: `assert_operable_applies_to` pins each of delete/archive/unarchive `applies_to` (entity e2e) and `app.inspect` `applies_to` (app-shell e2e) to Rust `COPYABLE_ENTITY_TYPES`; `assert_clipboard_applies_to` retained (delegates) so the clipboard trio guard still passes; both anchor that `field`/`view`/`perspective` are excluded.
- [x] `cargo test -p swissarmyhammer-command-service` passes (field-target assertions FAILED before the `applies_to` additions, PASS after). The only remaining failure is the pre-existing, unrelated `meta_tree_id_param_is_required_where_expected` (confirmed failing without these changes).

## Workflow
- Used `/tdd` — wrote the failing `list command` field-target assertions first, watched them fail (commands leaked), then added the `applies_to` declarations to make them pass. #field-moniker-fix #commands #entity-commands

## Review Findings (2026-06-14 13:20)

### Warnings
- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs:1501` and `crates/swissarmyhammer-command-service/tests/integration/builtin_entity_commands_e2e.rs:930` — Near-verbatim duplicated `assert_operable_applies_to` helper. RESOLVED: hoisted a single `pub fn assert_operable_applies_to(cmd, id)` into `tests/integration/support.rs` (which now imports `COPYABLE_ENTITY_TYPES` from `swissarmyhammer_kanban::commands::clipboard_commands`). Both e2e modules now `use crate::support::assert_operable_applies_to` and dropped their local copies; `assert_clipboard_applies_to` in the entity e2e still delegates to the shared helper. The now-unused `COPYABLE_ENTITY_TYPES` imports in both e2e files were removed. `cargo test -p swissarmyhammer-command-service --test integration` is green (50 passed, 0 failed).

## Review Findings (2026-06-14 14:05)

Prior warning re-verified as genuinely resolved: a single `pub fn assert_operable_applies_to` lives at `tests/integration/support.rs:314` (sole `COPYABLE_ENTITY_TYPES` import at `support.rs:31`); both e2e files import it (`builtin_entity_commands_e2e.rs:50`, `builtin_app_shell_commands_e2e.rs:100`) and call it (entity:698/731/766, app-shell:1489); `assert_clipboard_applies_to` (entity:904) delegates via a one-line body; no duplicate copies or stray imports remain. All four acceptance criteria re-confirmed against the code. Tests green: `--test integration` 50 passed / 0 failed, `--test list_applies_to` 6 passed / 0 failed; the only suite failure is the known-ignored pre-existing `meta_tree_id_param_is_required_where_expected` (card 01KV32F7NN642NX7E2M8SG5V54).

### Nits
- [x] `crates/swissarmyhammer-kanban/src/commands/clipboard_commands.rs:34` — Stale doc reference introduced by this change's rename: the doc comment on `COPYABLE_ENTITY_TYPES` names the TS constant as `CLIPBOARD_ENTITY_TYPES` in `builtin/plugins/entity-commands/index.ts`, but it was renamed to `OPERABLE_ENTITY_TYPES` (index.ts:86). The referenced identifier no longer exists. Fix: update the comment to `OPERABLE_ENTITY_TYPES`.
- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_entity_commands_e2e.rs:891` — Same stale reference in the `assert_clipboard_applies_to` doc: "TS `CLIPBOARD_ENTITY_TYPES` (`builtin/plugins/entity-commands/index.ts`)". Fix: update to `OPERABLE_ENTITY_TYPES`. (The `assert_clipboard_applies_to` *helper-name* references at `list_applies_to.rs:74/178` remain correct — only the two `CLIPBOARD_ENTITY_TYPES` constant mentions are stale.)