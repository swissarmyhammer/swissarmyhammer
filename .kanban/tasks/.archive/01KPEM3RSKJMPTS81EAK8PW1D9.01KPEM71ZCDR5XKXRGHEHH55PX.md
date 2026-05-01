---
assignees:
- claude-code
position_column: todo
position_ordinal: d080
title: 'Refactor command YAMLs: separate cross-cutting entity commands from type-specific commands'
---
## What

The command YAML organization is internally inconsistent. Cross-cutting commands (delete, archive, unarchive, cut, copy, paste) are split across `entity.yaml` (the cross-cutting file) *and* type-specific entity schemas (`task.yaml`, `tag.yaml`, `project.yaml`). The split is not driven by any rule, and the resulting gaps produce real bugs.

### Concrete symptoms (observed on the current branch)

1. **Delete project does not work.** `swissarmyhammer-kanban/builtin/entities/project.yaml` declares a type-specific `project.delete` command. Its Rust impl (`swissarmyhammer-kanban/src/commands/project_commands.rs::DeleteProjectCmd::available`) only checks `ctx.has_in_scope("project") || ctx.arg("id")`, ignoring the `target` moniker that `emit_entity_schema_commands` passes. When the context menu fires on a project card, `available()` returns false and the dispatch fails. The generalized `entity.delete` in `entity_commands.rs::DeleteEntityCmd` already handles project deletion by moniker, but project.yaml does not list `entity.delete` in its commands, so it never surfaces.

2. **No delete tag.** `swissarmyhammer-kanban/builtin/entities/tag.yaml` lists no delete command at all. `entity.delete` in `entity.yaml` *does* dispatch `tag` (see `DeleteEntityCmd` match arm), but it's `visible: false` and the tag schema does not redeclare it, so it never appears in palette or context menus.

3. **`entity.archive` with `scope: "entity:task"` in `entity.yaml`.** `swissarmyhammer-commands/builtin/commands/entity.yaml` declares `entity.archive` scope-restricted to task, but `tag.yaml` and `project.yaml` each redeclare `entity.archive` to make it appear on their entity — defeating the whole point of a cross-cutting command. Same for `entity.unarchive`.

4. **`entity.cut` / `entity.copy` / `entity.paste` live in `task.yaml`.** These are supposed to be cross-cutting (clipboard over any entity), but each entity YAML has to redeclare them to get the keybindings and `context_menu: true` on its type. `entity.yaml` also declares the same three commands, with `scope: "entity:task"` — again, pinned to task.

### The real problem

There is no documented rule for what belongs in `entity.yaml` vs. in a type-specific `<entity>.yaml`. Both act partly as registries and partly as overlays. Cross-cutting commands get re-declared per entity because the per-entity YAML is the only place where `context_menu` and `keys` take effect for that entity type (see `emit_entity_schema_commands` in `scope_commands.rs:570`).

### Goal

Define the rule, then align the YAMLs and Rust availability checks to it.

**Proposed rule** (confirm or revise during implementation):

- `swissarmyhammer-commands/builtin/commands/entity.yaml` is the **declaration** file for cross-cutting commands: `entity.add`, `entity.delete`, `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste`, `entity.update_field`. No `scope:` pinning here.
- Type-specific command files (`swissarmyhammer-commands/builtin/commands/<noun>.yaml`, e.g. `drag.yaml`, `attachment.yaml`) hold **type-specific** commands — operations that only apply to one entity type (e.g. `attachment.open`, `column.reorder`, `task.move`, `task.doThisNext`, `task.untag`, `tag.update`).
- Per-entity schemas (`swissarmyhammer-kanban/builtin/entities/*.yaml`) list **which** commands appear on that entity, plus the per-entity overlay metadata (`context_menu`, `keys`, `menu`, templated `name`). They do not re-declare the command contract — they reference a command `id` that lives in `builtin/commands/*.yaml`.
- A type-specific command is placed in its entity's YAML `commands:` list (e.g. `tag.update` in `tag.yaml`), not in `entity.yaml`.

### Files to touch

- `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove `scope: "entity:task"` from `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste`. Remove the per-task nouns (`task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `tag.update`, `column.reorder`, `attachment.delete`) that leaked in — move them to a new `swissarmyhammer-commands/builtin/commands/task.yaml` (and the appropriate existing files for non-task ones).
- `swissarmyhammer-kanban/builtin/entities/task.yaml` — drop re-declarations of `entity.cut`, `entity.copy`, `entity.paste`, `entity.archive`, `entity.unarchive`. Keep the reference entries (just `id`, plus any per-task overlay like `context_menu`, `keys`). Keep task-specific commands (`task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `attachment.delete`).
- `swissarmyhammer-kanban/builtin/entities/tag.yaml` — reference `entity.delete` so it surfaces on tags. Drop the full re-declaration of `entity.archive`/`entity.copy`/`entity.cut` (keep just the ID + overlay). Keep `tag.update`.
- `swissarmyhammer-kanban/builtin/entities/project.yaml` — replace `project.delete` with a reference to `entity.delete`. Drop the full re-declaration of `entity.archive`. Delete `DeleteProjectCmd` and its registration if nothing else uses it (audit first).
- `swissarmyhammer-kanban/src/commands/entity_commands.rs` — audit `ArchiveEntityCmd` / `UnarchiveEntityCmd` availability: `ArchiveEntityCmd::available` is already target-based, but confirm it works for all types (tag, project, column, actor) after the YAML is cleaned up.
- `swissarmyhammer-kanban/src/commands/project_commands.rs` — remove `DeleteProjectCmd` (or keep it as a palette-only variant that reads `id` from scope chain, if the palette flow needs it separately from the context-menu flow).
- `swissarmyhammer-kanban/src/commands/mod.rs` — update `register_commands()` registrations to match whatever is removed.
- `swissarmyhammer-kanban/src/scope_commands.rs` — entity-schema command emission (`emit_entity_schema_commands`) and the `collect_entity_schema_cmds` helper may need a small change: when an entity schema lists a command by `id` only (no redeclared contract), the emitter should still look up the registry def for `context_menu`/`keys` defaults and let the entity override specific fields. Verify the existing `EntityCommand` struct already supports this — it does (all fields are optional) — but confirm the merge semantics are clearly defined and tested.

### Subtasks

- [ ] Document the cross-cutting vs type-specific rule in a short comment at the top of `swissarmyhammer-commands/builtin/commands/entity.yaml` and in the CLAUDE.md or a new reference memory.
- [ ] Move task-specific nouns out of `entity.yaml` into a new `swissarmyhammer-commands/builtin/commands/task.yaml` (and confirm the YAML loader picks it up via `builtin_yaml_sources()` / `include_dir!`).
- [ ] Remove `scope: "entity:task"` from the cross-cutting commands in `entity.yaml`.
- [ ] Clean up per-entity YAMLs (`task.yaml`, `tag.yaml`, `project.yaml`, `column.yaml`, `board.yaml`, `actor.yaml`, `attachment.yaml`) so each only lists commands that actually apply — with overlay-only overrides, not redeclarations.
- [ ] Add `entity.delete` reference to `tag.yaml` so delete tag works from context menu.
- [ ] Replace `project.delete` with `entity.delete` reference in `project.yaml`; retire `DeleteProjectCmd` + `project_commands.rs` if unused.
- [ ] Verify every entity that should support archive gets `entity.archive` and `entity.unarchive` via its schema (tag, project, column as appropriate — attachments do not archive).

## Acceptance Criteria

- [ ] Right-click on a project card shows "Delete Project" and the action actually deletes the project.
- [ ] Right-click on a tag shows "Delete Tag" and the action deletes the tag.
- [ ] Right-click on a tag shows "Archive Tag" — no `scope: "entity:task"` blocking it.
- [ ] Cross-cutting commands (`entity.delete`, `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste`, `entity.add`, `entity.update_field`) appear exactly once in `swissarmyhammer-commands/builtin/commands/entity.yaml` with no `scope: "entity:<type>"` pinning.
- [ ] No type-specific commands (`task.*`, `tag.*`, `project.*`, `column.*`, `attachment.*`) live in `entity.yaml`.
- [ ] Per-entity YAMLs list command `id`s with overlay fields only — no duplicate `name`/`params` declarations that shadow the registry definition.
- [ ] `test_all_yaml_commands_have_rust_implementations` in `mod.rs` still passes (every YAML-declared command has a Rust impl).
- [ ] Grep shows no `"entity.archive"`, `"entity.cut"`, `"entity.copy"`, `"entity.paste"` with full `params:` or `undoable:` blocks inside `builtin/entities/*.yaml` — only references with overlay metadata.

## Tests

- [ ] Add integration tests in `swissarmyhammer-kanban/src/scope_commands.rs` that assert delete/archive are emitted with `available: true` for each applicable entity type:
  - `delete_entity_surfaces_on_project_context_menu`
  - `delete_entity_surfaces_on_tag_context_menu`
  - `archive_entity_surfaces_on_tag_context_menu`
  - `archive_entity_surfaces_on_project_context_menu`
- [ ] Update `swissarmyhammer-kanban/src/commands/entity_commands.rs` tests:
  - Add `delete_entity_deletes_project` matching the existing `delete_entity_deletes_tag` pattern (there is no project test today even though the match arm exists).
  - Keep the archive tests; add one that archives a tag via `entity.archive` end-to-end.
- [ ] Update or retire `project_commands.rs` tests along with `DeleteProjectCmd`.
- [ ] Add a YAML-hygiene test that parses every `builtin/entities/*.yaml` and asserts: for any command whose `id` starts with `entity.`, the entry contains no `params:` field (entity-schema entries are overlays only, not declarations).
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands commands::` — all tests green.
- [ ] Manual smoke test in `kanban-app`: right-click a project, tag, and task, confirm delete/archive/cut/copy/paste appear correctly per type and actually execute.

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
- Start with the hygiene test and the two delete-from-context-menu tests; they will fail on the current branch and drive the YAML cleanup.
- Do the architectural doc comment as a separate small commit so the rule is captured before the mechanical moves. #commands