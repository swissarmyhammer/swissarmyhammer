---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8b80
title: 'Entity context menu: group Cut/Copy/Paste and Delete/Archive with stable ordering and separator'
---
## What

Right-click context menus on any entity currently emit `entity.cut`, `entity.copy`, `entity.paste`, `entity.delete`, `entity.archive`, `entity.unarchive`, `ui.inspect` as one undifferentiated block with **no separator and no stable order**. Two defects to fix at once:

1. **No grouping** — all cross-cutting commands get the same `ResolvedCommand.group` string in `swissarmyhammer-kanban/src/scope_commands.rs` (`group: entity_type.to_string()`). The frontend renderer at `kanban-app/ui/src/lib/context-menu.ts:53-69` inserts a separator only when `cmd.group !== lastGroup`, so with one group there are zero separators.
2. **No stable order** — `emit_cross_cutting_commands` iterates `all_registry_cmds` which is backed by a `HashMap<String, CommandDef>`. HashMap iteration is unordered and Rust's `DefaultHasher` reseeds per process, so the menu order differs run to run.

**Desired context-menu order** (user-specified + logical extension to cover all cross-cutting commands):
```
Cut <Entity>
Copy <Entity>
Paste <Entity>
──────────────
Delete <Entity>
Archive <Entity>
Unarchive <Entity>        (hidden by Command::available when N/A)
──────────────
Inspect <Entity>
```

## Approach

**Design choice**: Add two new optional CommandDef fields — `context_menu_group: Option<u32>` and `context_menu_order: Option<u32>` — and use them to drive sort + separator placement in the cross-cutting emitter. Chosen over reusing `menu.group`/`menu.order` because:
- `MenuPlacement` couples grouping to native-menu-bar placement (requires a `path`). Cut/copy/paste are in the native Edit menu, but delete/archive/unarchive and inspect are not — and making them appear there silently is an unwanted side-effect.
- Keeps the two surfaces (native menu bar vs context menu) independently controllable.
- The field names state the intent explicitly at each call site.

## Acceptance Criteria

- [x] Right-clicking a task/tag/column/any entity shows commands in this order: `Cut`, `Copy`, `Paste`, `─── separator ───`, `Delete`, `Archive` (and `Unarchive` when available), `─── separator ───`, `Inspect <Entity>`.
- [x] Separators appear between the Cut/Copy/Paste block and the Delete/Archive block, and between the Delete/Archive block and Inspect.
- [x] Order is identical across process restarts (no HashMap-hash reshuffling). Verified by `cross_cutting_order_is_stable_across_runs`.
- [x] The command palette is unaffected — palette entries are emitted via the same pipeline but the frontend palette renderer ignores `group`. No visible change there.
- [x] The native menu bar (Edit → Cut/Copy/Paste) is unaffected — `menu.group`/`menu.order` still drive that placement via `kanban-app/src/menu.rs::append_grouped_entries`.
- [x] `#[serde(deny_unknown_fields)]` on `CommandDef` still holds — the new optional fields are serde-defaultable so existing YAMLs without them parse unchanged.

## Tests

- [x] New Rust test `cross_cutting_context_menu_is_ordered_and_grouped` in `swissarmyhammer-kanban/src/scope_commands.rs` tests module — asserts order and group strings.
- [x] New Rust test `cross_cutting_order_is_stable_across_runs` in the same test module — guards the intra-process determinism invariant.
- [x] Updated `swissarmyhammer-commands/src/types.rs` `command_def_yaml_round_trip` test to exercise the new fields.
- [x] Existing tests still pass:
  - `swissarmyhammer-commands/src/registry.rs` — `test_perspective_yaml_parses`, `keymap_commands_are_visible_in_palette`, merge tests pass
  - `swissarmyhammer-kanban/tests/command_dispatch_integration.rs` — 45/45 pass
  - `kanban-app/ui/src/lib/context-menu.test.tsx` — 9/9 pass
- [x] All `scope_commands::tests::*` (84) pass.
- [x] All `command_surface_matrix` (50) pass.

Note: Two pre-existing test failures are unrelated to this task (confirmed via `git stash` reproduction):
- `swissarmyhammer-commands::registry::tests::builtin_yaml_files_parse` — hardcoded count 62 but only 61 commands since someone retired `attachment.delete`.
- `swissarmyhammer-kanban::commands::entity_commands::tests::delete_entity_deletes_attachment_via_scope_chain` — attachment-delete refactor test.

#ux #commands #bug