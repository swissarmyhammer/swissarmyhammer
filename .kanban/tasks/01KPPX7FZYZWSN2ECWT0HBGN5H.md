---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff8280
title: 'Commands: fix board entity.delete / entity.archive contract gap'
---
## What

`DeleteEntityCmd` and `ArchiveEntityCmd` both return `available: true` when the target is a board moniker (`board:{id}`), but neither command handles the `"board"` entity type in its `execute()` match arms:

- `DeleteEntityCmd::execute` — matches `task | tag | column | actor | project`, and the catch-all returns `CommandError::ExecutionFailed("unknown entity type for delete: 'board'")`.
- `ArchiveEntityCmd::execute` — calls `ectx.archive("board", id)` generically. Depending on `EntityContext::archive` semantics this may silently succeed (moving the board file to `.archive/`) which is _not_ a UX we want — archiving a board is a no-op that leaves the app in an undefined state.

The surface says "you can click this"; dispatch either fails loudly or mutates state in a way no code path treats as meaningful. Both are latent bugs.

## Why

Tracked as review-finding follow-up on 01KPEMFBBFRE1JWRJ9AXQFVSEB. The matrix test (`swissarmyhammer-kanban/tests/command_surface_matrix.rs`) currently pins the "surface but dispatch-fail" contract at `matrix_board_delete_available` and `matrix_board_archive_available` — it's correct for the verification task, but the underlying behaviour should change.

## Acceptance Criteria

- [ ] `DeleteEntityCmd::available()` returns `false` for board monikers (add a per-type opt-out list, or explicit `entity_type == "board"` check).
- [ ] `ArchiveEntityCmd::available()` returns `false` for board monikers.
- [ ] Update `matrix_board_delete_available` and `matrix_board_archive_available` in `swissarmyhammer-kanban/tests/command_surface_matrix.rs` to use `assert_absent` — rename them to `matrix_board_delete_not_available` / `matrix_board_archive_not_available` to match the other `_not_available` tests.
- [ ] Regenerate any snapshot fixtures affected by the surface change (`UPDATE_SNAPSHOTS=1 cargo test -p swissarmyhammer-kanban --test command_snapshots`).
- [ ] All pre-existing matrix + snapshot + keybinding + crate tests still pass.

## Files to touch

- `swissarmyhammer-kanban/src/commands/entity_commands.rs` — extend `DeleteEntityCmd::available()` and `ArchiveEntityCmd::available()` with the opt-out.
- `swissarmyhammer-kanban/tests/command_surface_matrix.rs` — rename + flip assertions for the two board tests.
- `swissarmyhammer-kanban/tests/snapshots/` — regenerate the board snapshots (if the surface shrinks).

## Design notes

Consider whether other entity types also need opt-outs — e.g. actor-archive, column-archive — which the matrix currently pins as "surface with available: true" despite the card marking them "—". A consistent opt-out policy ("boards, columns, actors never archive/delete via the cross-cutting command; use their type-specific commands") is cleaner than ad-hoc per-test pinning.

#commands