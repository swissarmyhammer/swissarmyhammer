---
assignees:
- claude-code
depends_on:
- 01KPEPTF55TWT0NB6SGX2JWJ6T
position_column: todo
position_ordinal: d780
title: 'Commands: add emit_cross_cutting_commands pass driven by existing from:target params; migrate ui.inspect as pilot'
---
## What

Add the single emission pass that surfaces cross-cutting commands on every entity moniker in scope — without inventing a new flag. The signal is already in the YAML: any command whose primary param declares `from: target` operates on a target entity, so emit it once per entity moniker with `target = moniker`. Migrate `ui.inspect` to prove the pattern.

### Why no new flag

A `per_entity: true` boolean on `CommandDef` would duplicate information the params already encode. `entity.delete`, `entity.archive`, `entity.unarchive` each declare `params: [{name: moniker, from: target}]`. That declaration is the contract: "this command's subject comes from the context-menu target." The emitter reads the declaration and acts on it. No flag to drift out of sync.

### Scope-chain thinking

The existing `emit_scoped_commands` in `swissarmyhammer-kanban/src/scope_commands.rs` walks the scope chain and for each entity moniker runs two passes: entity-schema commands and scoped-registry commands. That's exactly the right shape — the new cross-cutting pass slots alongside them.

The scope chain already carries everything needed: innermost-first monikers, each parseable into `entity_type` + `entity_id`. Walk it once, and for each entity moniker, emit every registry command that:

1. Has a param with `from: target` (or the emitter's moral equivalent — confirm the existing `ParamSource` enum in `swissarmyhammer-commands`).
2. Has either no `scope:` pin or a pin that matches the moniker's entity type.
3. Optionally constrains `entity_type` on the target param — if present, only emit for monikers whose type matches.
4. Passes `check_available(cmd_id, scope_chain, Some(moniker), ...)` — Rust `available()` is the final opt-out (e.g. `DeleteEntityCmd::available` can reject attachments).

Dedup via the existing `(id, target)` seen set so a command never double-emits for the same target.

### Pilot migration — `ui.inspect`

`ui.inspect` today declares `params: [{name: moniker, from: args}]` and `visible: false`. That is why it never auto-emits and why six entity schemas opt it in by hand. Fix the declaration:

```yaml
- id: ui.inspect
  name: "Inspect {{entity.type}}"
  context_menu: true
  params:
    - name: moniker
      from: target
```

Drop `visible: false`. The new emission pass picks it up and surfaces it on every entity moniker. Delete the six redundant entity-schema blocks.

### Files to touch

- `swissarmyhammer-commands/src/registry.rs` (or wherever `CommandDef`/`ParamDef`/`ParamSource` live) — confirm the source tagging exists; read-only unless the existing schema can't already express `from: target` cleanly.
- `swissarmyhammer-commands/builtin/commands/ui.yaml` — fix `ui.inspect` declaration as above.
- `swissarmyhammer-kanban/src/scope_commands.rs` — add `emit_cross_cutting_commands` (or `emit_target_driven_commands` — pick a name that reflects the signal) and wire it into `commands_for_scope`. Document the ordering in a module comment: entity-schema → cross-cutting → scoped-registry → global-registry → dynamic.
- `swissarmyhammer-kanban/builtin/entities/task.yaml`, `tag.yaml`, `project.yaml`, `column.yaml`, `board.yaml`, `actor.yaml` — delete the six `ui.inspect` entries.
- `swissarmyhammer-kanban/src/commands/ui_commands.rs::InspectCmd` — confirm `available()` and `execute()` both read from `ctx.target` now that the command is target-driven.

### Subtasks

- [ ] Confirm the existing `ParamDef`/`ParamSource` schema cleanly expresses "from target"; no new fields added unless necessary.
- [ ] Implement `emit_cross_cutting_commands` driven by the target-param signal.
- [ ] Wire into `commands_for_scope`; document emission ordering.
- [ ] Promote `ui.inspect` to target-driven in `ui.yaml`; delete six entity-schema copies.

## Acceptance Criteria

- [ ] No `per_entity` / `cross_cutting` field added to `CommandDef`.
- [ ] `emit_cross_cutting_commands` exists and is called once per `commands_for_scope` invocation.
- [ ] Right-click on every entity type (task, tag, project, column, board, actor) shows "Inspect <Type>" and opens the inspector.
- [ ] `grep -n 'id: ui\\.inspect' swissarmyhammer-kanban/builtin/entities/` returns zero matches.
- [ ] `ui.inspect` is declared exactly once in the registry.

## Tests

- [ ] Add `ui_inspect_auto_emits_on_every_entity_type` in `scope_commands.rs` tests — parameterized over `["task:01X", "tag:01T", "project:backend", "column:todo", "board:main", "actor:alice"]`. Each scope yields a `ResolvedCommand` with `id == "ui.inspect"` and `target == Some(moniker)`.
- [ ] Add `cross_cutting_dedupes_per_target` — scope `["task:01X", "column:todo", "board:main"]` emits `ui.inspect` exactly once per distinct target, never duplicated.
- [ ] Add `cross_cutting_respects_available_opt_out` — stub command whose `available()` returns false for a given moniker type does NOT emit for it.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-commands -p swissarmyhammer-kanban scope_commands` — all green.

## Workflow

- Use `/tdd` — `ui_inspect_auto_emits_on_every_entity_type` fails until the pass exists AND the entity schemas drop their opt-ins.

#commands

Depends on: 01KPEMA771EPB8V51SPKAE0PBB (scope pins must be off so the cross-cutting pass emits without fighting `scope_matches`)