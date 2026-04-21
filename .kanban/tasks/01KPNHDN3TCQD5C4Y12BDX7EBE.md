---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff80
title: 'Commands: actually generalize CopyCmd/CutCmd/PasteCmd to read ctx.target (blocks tag.yaml + task.yaml cleanup)'
---
## What

Card 01KPG5XK61ND4JKXW3FCM3CC97 ("Commands: generalize copy/cut to work on any entity type") and 01KPG5YB7GTQ6Q3CEQAMXPJ58F ("Commands: paste dispatcher mechanism") were marked done but the legacy `CopyCmd` / `CutCmd` / `PasteCmd` in `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` were NOT generalized. They still dispatch via `ctx.has_in_scope("tag") || ctx.has_in_scope("task")` and call `tag::CopyTag` / `task::CopyTask` directly. The YAML for `entity.copy` / `entity.cut` / `entity.paste` in `swissarmyhammer-commands/builtin/commands/entity.yaml` still declares `params: [{name: task|column, from: scope_chain, entity_type: ...}]` instead of `{name: moniker, from: target}`.

This blocks two cleanup cards:
- 01KPG6XPMDHSH8PMD248YK6KAK (tag.yaml cleanup) — removing `entity.archive/copy/cut` from tag.yaml entity breaks 4 scope_commands tests because the cross-cutting auto-emit pass requires `from: target` to surface these on a tag scope.
- 01KPG6XDVSY9DAN2TS26W52NN6 (task.yaml cleanup) — same issue.

### Required changes

1. **`swissarmyhammer-commands/builtin/commands/entity.yaml`** — change `entity.copy`, `entity.cut`, `entity.archive`, `entity.unarchive` params to `[{name: moniker, from: target}]` (entity.archive/unarchive already done; copy/cut still on `from: scope_chain`). `entity.paste` is more nuanced — its destination comes from scope chain in the legacy impl; the new `PasteEntityCmd` (matrix dispatcher) reads `ctx.target` instead, so registration needs to flip from `PasteCmd` to `PasteEntityCmd` AND param needs to be `{name: moniker, from: target}`.

2. **`swissarmyhammer-kanban/src/commands/clipboard_commands.rs`** — `CopyCmd`/`CutCmd` rewrite:
   - `available()` returns true when `ctx.target` parses to a moniker of a known entity type (task/tag/project/column/board/actor).
   - `execute()` reads `ctx.target` moniker, parses `entity_type:id`, dispatches to the entity-specific copy/cut operation. Need polymorphic dispatch — currently only `task::CopyTask` and `tag::CopyTag` exist. Each entity type needs a copy implementation that snapshots fields into the clipboard payload via `EntityContext::read`.
   - Delete the legacy task-only/tag-only branching.

3. **`swissarmyhammer-kanban/src/commands/mod.rs`** — verify `entity.paste` is registered to `PasteEntityCmd` (matrix dispatcher), not legacy `PasteCmd`. Delete legacy `PasteCmd` once `PasteEntityCmd` covers all paths.

4. **Update tests** — the 4 scope_commands tests that depend on entity.copy/cut surfacing on tag scope via per-entity opt-ins need to either be deleted (if they're stale) or rewritten to assert auto-emit instead.

### Acceptance criteria

- [x] `entity.copy` / `entity.cut` declare `from: target` in entity.yaml.
- [x] `CopyCmd::available` returns true for any known entity moniker target.
- [x] After copying a tag with target `tag:X`, `UIState::clipboard_payload()` contains the tag's type, id, and fields.
- [x] After this card lands, removing `entity.copy/cut/archive` from `swissarmyhammer-kanban/builtin/entities/{tag,task}.yaml` does NOT break any scope_commands tests (tag.yaml + task.yaml cleanup cards become unblocked).
- [x] Legacy `PasteCmd` is deleted; only `PasteEntityCmd` (matrix dispatcher) handles `entity.paste`.

### Tests

- [x] Add `copy_entity_works_on_tag_via_target` — dispatch entity.copy with target `tag:X`, assert clipboard contains tag fields.
- [x] Add `copy_entity_works_on_project_via_target`, `copy_entity_works_on_column_via_target`, `copy_entity_works_on_actor_via_target`.
- [x] Existing copy/cut tests updated to use target-driven availability.
- [x] `cargo nextest run -p swissarmyhammer-kanban` — all green except the deliberately-RED hygiene test.

### Workflow

Use `/tdd`. Write `copy_entity_works_on_tag_via_target` first; it fails until the generalization lands.

#commands

This is the corrective task for un-finished work in 01KPG5XK61ND4JKXW3FCM3CC97 and 01KPG5YB7GTQ6Q3CEQAMXPJ58F. Once this lands, 01KPG6XPMDHSH8PMD248YK6KAK and 01KPG6XDVSY9DAN2TS26W52NN6 unblock.

### Implementation notes (2026-04-20 corrective pass)

State found on dispatch:
- `entity.yaml` — `entity.copy/cut/paste/archive/unarchive` already declared `params: [{name: moniker, from: target}]`. No edit needed.
- `clipboard_commands.rs` — `CopyEntityCmd`, `CutEntityCmd`, `PasteEntityCmd` (matrix dispatcher) already implemented and tested; the requested `copy_entity_works_on_{tag,task,project,column,actor}_via_target` tests already in place.
- `mod.rs::register_clipboard` — already wires `entity.paste` to `PasteEntityCmd::new()`.
- `tag.yaml` and `task.yaml` — neither lists `entity.copy/cut/archive`. The auto-emit pass surfaces them based on the entity's moniker via the cross-cutting registry.

Cleanup applied this pass:
- Deleted dead aliases `pub use CopyEntityCmd as CopyCmd; pub use CutEntityCmd as CutCmd;` from `clipboard_commands.rs` (no consumers).
- Updated `swissarmyhammer-kanban/tests/command_dispatch_integration.rs::entity_copy_copies_task_to_clipboard` to dispatch with `target: Some("task:<id>")` instead of stale `scope: ["task:<id>"]` — the integration test was the last consumer of legacy scope-driven copy.
- Updated 5 stale availability tests in `commands/mod.rs` (`paste_*_available_with_*`, `paste_not_available_without_scope`) to drive `entity.paste` via `target` rather than `scope`, matching the production `from: target` contract.
- Replaced the placeholder `cut_tag_not_available_without_task_parent` assertion in `scope_commands.rs` with the now-correct invariant: `entity.cut` on a `tag:X` target without a task in scope must NOT surface (no destructive op exists).
- Refreshed two doc comments in `scope_commands.rs` referencing `PasteCmd::available()` to `PasteEntityCmd::available()`.

Verification:
- `cargo nextest run -p swissarmyhammer-kanban` — 1145/1145 passed.
- `cargo nextest run -p swissarmyhammer-commands` — 175/175 passed.
- `cargo nextest run -p swissarmyhammer-kanban scope_commands` — 82/82 passed (acceptance test).
- `cargo clippy -p swissarmyhammer-kanban --all-targets` — clean.