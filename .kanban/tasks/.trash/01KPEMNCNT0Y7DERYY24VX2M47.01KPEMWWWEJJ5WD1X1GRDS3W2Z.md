---
assignees:
- claude-code
depends_on:
- 01KPEM811W5XE6WVHDQVRCZ4B0
position_column: todo
position_ordinal: d780
title: 'Commands: make ui.inspect a true cross-cutting command — declare once, auto-emit on every entity moniker'
---
## What

`ui.inspect` is declared with `visible: false` in `swissarmyhammer-commands/builtin/commands/ui.yaml`, then redeclared in every entity schema (`tag.yaml`, `task.yaml`, `project.yaml`, `column.yaml`, `board.yaml`, `actor.yaml`) as an identical 3-line block:

```yaml
- id: ui.inspect
  name: "Inspect {{entity.type}}"
  context_menu: true
```

Only `attachment.yaml` omits it. Six copies of the same YAML exist only because the scope_commands emitter (`swissarmyhammer-kanban/src/scope_commands.rs::emit_scoped_registry_commands`) skips commands with `visible: false`, so the per-entity opt-in is the only surface path.

Inspect is a textbook cross-cutting entity command — it applies to any entity by target moniker. This card collapses the six copies into a single declaration and adds a generic auto-emit path so *any* cross-cutting command surfaces on *any* entity moniker automatically.

### Design

1. Declare `ui.inspect` once in `swissarmyhammer-commands/builtin/commands/ui.yaml` with the template name, `context_menu: true`, and no `visible: false`:

    ```yaml
    - id: ui.inspect
      name: "Inspect {{entity.type}}"
      context_menu: true
      params:
        - name: moniker
          from: target
    ```

2. Add a "cross-cutting" signal to the command declaration — either a new boolean field on `CommandDef` (e.g. `per_entity: true`) OR a naming convention (`entity.*` and specific known IDs like `ui.inspect`). Prefer an explicit flag for clarity.

3. In `swissarmyhammer-kanban/src/scope_commands.rs`, add a `emit_cross_cutting_commands` pass inside `emit_scoped_commands`. For each entity moniker in the scope chain and each registry command with the cross-cutting flag, emit a `ResolvedCommand` with `target: Some(entity_moniker)` and the resolved templated name. Dedup via the existing `(id, target)` seen set.

4. Remove `ui.inspect` entries from every entity schema YAML. Attachment stays as-is (inspecting an attachment opens the file, which is a different command — confirm with `AttachmentOpenCmd`).

5. Availability per entity is enforced at the Rust `available()` level — the `InspectCmd` impl in `swissarmyhammer-kanban/src/commands/ui_commands.rs` already checks `ctx.target.is_some() || !ctx.scope_chain.is_empty()`, so it naturally applies to any entity moniker.

### Files to touch

- `swissarmyhammer-commands/builtin/commands/ui.yaml` — promote `ui.inspect` to context_menu: true, un-hide, add moniker param.
- `swissarmyhammer-commands/src/registry.rs` (or wherever `CommandDef` is defined) — add the cross-cutting flag field (default false). Check existing struct first: grep for `pub struct CommandDef`.
- `swissarmyhammer-kanban/src/scope_commands.rs` — add `emit_cross_cutting_commands`; wire into `emit_scoped_commands` so it runs once per entity moniker.
- `swissarmyhammer-kanban/builtin/entities/task.yaml`, `tag.yaml`, `project.yaml`, `column.yaml`, `board.yaml`, `actor.yaml` — remove the `ui.inspect` entries entirely.
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — confirm `InspectCmd::execute` reads from `ctx.target` when that's the dispatch source.

### Subtasks

- [ ] Promote `ui.inspect` declaration in ui.yaml; add the cross-cutting flag.
- [ ] Implement `emit_cross_cutting_commands` and wire it in.
- [ ] Remove the six redundant entries from entity schema YAMLs.
- [ ] Write a unit test that asserts `ui.inspect` emits with `target: Some("<entity_type>:<id>")` for task, tag, project, column, board, and actor without any per-entity opt-in.

## Acceptance Criteria

- [ ] `ui.inspect` is declared in exactly one place (`ui.yaml`).
- [ ] `grep -n 'id: ui\\.inspect' swissarmyhammer-kanban/builtin/entities/` returns no matches.
- [ ] Right-clicking any entity type (task, tag, project, column, board, actor) still shows an "Inspect <Type>" context-menu item that opens the inspector.
- [ ] The cross-cutting flag exists on `CommandDef` and can be set on other cross-cutting commands (`entity.delete`, `entity.archive`, etc.) in follow-up work.
- [ ] Availability is target-driven: `commands_for_scope` produces `ui.inspect` with `target: Some(moniker)` not `target: None` for each entity in scope.

## Tests

- [ ] Add `ui_inspect_emits_on_every_entity_type` in `swissarmyhammer-kanban/src/scope_commands.rs` tests: parameterize across `["task:01X", "tag:01T", "project:backend", "column:todo", "board:main", "actor:alice"]`, assert each produces a `ResolvedCommand` with `id == "ui.inspect"` and `target == Some(moniker)`.
- [ ] Add `ui_inspect_not_duplicated_when_multiple_entities_in_scope` — with a deep scope chain (`["task:01X", "column:todo", "board:main"]`), assert `ui.inspect` appears exactly once per distinct target, not merged.
- [ ] Update any existing test that relies on `ui.inspect` being in an entity schema to now rely on auto-emission.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands ui_commands` — all tests green.

## Workflow

- Use `/tdd` — write `ui_inspect_emits_on_every_entity_type` first; it fails on the current branch because removing the entity schema entry would make inspect disappear.

#commands

Notes: This card establishes the "cross-cutting command auto-emit" pattern. A follow-up should migrate `entity.delete`, `entity.archive`, `entity.unarchive`, `entity.copy`, `entity.cut`, `entity.paste` to the same mechanism, retiring the overlay-only per-entity entries planned in cards 01KPEMBDRYBY21G6NE4EZ7A4EC / 01KPEMCTST7QGRZB9T99Z8H04X / 01KPEME1897275TKE61EKN6EVX. Consider calling `/plan` to revise those three cards once this mechanism is in place.