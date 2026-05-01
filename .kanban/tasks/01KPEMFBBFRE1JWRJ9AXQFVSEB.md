---
assignees:
- claude-code
depends_on:
- 01KPG6XDVSY9DAN2TS26W52NN6
- 01KPG6XPMDHSH8PMD248YK6KAK
- 01KPG6XZ9GKP2VJPA6XWNE8WN4
- 01KPG6Y6WKHYH7EYDJ0NX8CR1R
- 01KPG6YDZDCPWGWKCC38TWM8AV
- 01KPG6YN15ECCK9SP262BJKGK2
- 01KPEME1897275TKE61EKN6EVX
- 01KPG7FDDG75EFABQ47Y198ZZJ
- 01KPG7FYGWK99011HCV89FGQTH
- 01KPG7GETZE5G707HD226XK9PV
- 01KPG7H3KJSDYQT7Y8ED8ADXX8
- 01KPG7HK6TJZ6C14JKZXH3KWRK
- 01KPG7J5H41P36A4ARFZ67Q909
- 01KPG7KH75NXGD65J1479HWMBN
- 01KPJSSVCW774TK2E2JSMD3Y1J
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffe80
title: 'Commands: automated verification — matrix test, snapshot tests, keybinding dispatch tests'
---
## What

Fully automate the verification of the cross-cutting command refactor. Every claim the plan makes about "right-click shows X" or "key chord Y fires Z" must be provable from a test runner, not from a human clicking around. Three layers of automation, each catching a different failure class.

### Layer 1 — Entity × command surface matrix

`swissarmyhammer-kanban/tests/command_surface_matrix.rs` (NEW integration test). For each (entity, command) cell in the matrix below, build a real `CommandContext` through the full `commands_for_scope` dispatch, then:

- Assert **presence** — the `ResolvedCommand` exists with the expected `id` and `target`.
- Assert **shape** — exact values of `name` (template-resolved), `context_menu`, `keys`, `menu` (when set), and `available`.
- Assert **absence** — for dashed cells, the command either does not appear or has `available: false`.

| Entity      | inspect | delete | archive | unarchive | copy | cut | paste |
|-------------|---------|--------|---------|-----------|------|-----|-------|
| task        | yes     | yes    | yes     | yes       | yes  | yes | yes (into column/board) |
| tag         | yes     | yes    | yes     | yes       | yes  | yes | yes (onto task) |
| project     | yes     | yes    | yes     | yes       | —    | —   | —     |
| column      | yes     | yes    | —       | —         | —    | —   | yes (task clipboard) |
| actor       | yes     | yes    | —       | —         | —    | —   | —     |
| board       | yes     | —      | —       | —         | —    | —   | yes (task clipboard) |
| attachment  | — (AttachmentOpenCmd is the inspect-equivalent) | — (attachment.delete is type-specific) | — | — | — | — | — |

Positive cells also call `execute()` against a real `KanbanContext` (like `entity_commands.rs::delete_entity_deletes_tag` does) and assert the expected state change.

### Layer 2 — Snapshot tests (regression net)

`swissarmyhammer-kanban/tests/command_snapshots.rs` (NEW, using `insta` or simple JSON file diffs — the crate already has serialization for `ResolvedCommand`). For each canonical scope, serialize the full `commands_for_scope` output to a snapshot file and commit it. Future refactors that silently reshape menus get caught by the snapshot diff.

Canonical scopes to snapshot:

- `["board:main"]` — empty board, shows board-level commands only
- `["column:todo", "board:main"]` — column context
- `["task:01X", "column:todo", "board:main"]` — task context
- `["tag:01T", "task:01X", "column:todo", "board:main"]` — tag-on-task context
- `["project:backend"]` — project context
- `["actor:alice"]` — actor context
- `["attachment:/tmp/x.png", "task:01X", "column:todo"]` — attachment context
- Each scope with `context_menu_only = true` and `false`

Snapshots are the most sensitive regression catcher — a missing command, a rename, a keybinding drift, a menu-ordering change all show up as a diff.

### Layer 3 — Keybinding dispatch tests (frontend)

`kanban-app/ui/src/lib/keybindings.test.ts` already exists. Extend it with explicit cases for every keybinding declared by a cross-cutting command:

- `vim: dd` on a task moniker → dispatches `entity.delete` (or the appropriate command for the scope)
- `vim: dd` on a tag moniker → dispatches `entity.delete` under the new plan
- `Mod+Backspace` on task/tag/project → `entity.delete`
- `Mod+C`/`Mod+X`/`Mod+V` on supported entities → entity.copy/cut/paste
- `Escape` → `ui.inspector.close`
- `Mod+K` → `ui.palette.open`

The existing test harness loads the real keybindings config and simulates the keystroke through the resolver; assert the resolved command ID matches. No browser required.

### Layer 4 — UIState round-trip (optional but cheap)

For each cross-cutting command that mutates state (delete, archive, paste), add a Rust integration test that:

1. Builds a real `KanbanContext` with fixture entities.
2. Calls `commands_for_scope` → gets the `ResolvedCommand`.
3. Dispatches the command with the same target.
4. Queries the `KanbanContext` and asserts the state change.

This is what `entity_commands.rs::delete_entity_deletes_tag` already does — just apply the pattern across every positive matrix cell. Delta from layer 1 is that this exercises the full surface+dispatch loop, not just surface OR dispatch in isolation.

### What this replaces

Delete "Manual smoke test" from the acceptance criteria entirely. It was the lazy answer. Every row in the matrix has a corresponding automated test; every keybinding has a dispatch test; every scope has a snapshot. A green `cargo nextest run` + green `bun test` is proof of correctness, not "I clicked around and it looked fine."

### Files to touch

- `swissarmyhammer-kanban/tests/command_surface_matrix.rs` (NEW)
- `swissarmyhammer-kanban/tests/command_snapshots.rs` (NEW)
- `swissarmyhammer-kanban/tests/snapshots/` (NEW directory for snapshot files, if using `insta`)
- `kanban-app/ui/src/lib/keybindings.test.ts` (extend)
- `swissarmyhammer-kanban/src/commands/entity_commands.rs` (extend with UIState round-trip cases where missing)

### Subtasks

- [x] Implement matrix test — one `#[tokio::test]` per entity row, table-driven over command columns. Expanded to 49 tests (7 entities including attachment × 7 commands) — one test per cell, so a failure names the exact cell.
- [x] Implement snapshot tests for the 7 canonical scopes × 2 filter modes. 14 snapshot files committed under `swissarmyhammer-kanban/tests/snapshots/`. Outputs are sorted by `(group, id, target)` so `HashMap` iteration order can't churn them. The live `board:{id}` is rewritten to `board:BOARD_ID_STABLE` in the snapshot for run-stability.
- [x] Extend keybinding tests with cross-cutting cases. Rewrote the `cross-cutting command keybinding dispatch` block in `kanban-app/ui/src/lib/keybindings.test.ts` — the earlier placeholder tests used a malformed scope shape and were failing. Now every keybinding declared on a cross-cutting command (`dd`, `Mod+C`/`Mod+X`/`Mod+V`, `Escape`, `Mod+K`, `Mod+Backspace`, and their vim duals `y`/`x`/`p`/`q`/`:`) has a test that runs the keystroke through `createKeyHandler` + `extractScopeBindings` and asserts the resolved command id.
- [x] Add UIState round-trip tests for mutating commands where the existing tests don't already cover them. Layer 1 is the round-trip — every positive cell in the matrix both surfaces the command AND executes it against a real `KanbanContext`, asserting the state change.
- [x] Confirm `yaml_hygiene_entity_schemas_have_no_commands_key` passes — verified green via `cargo test -p swissarmyhammer-kanban --lib yaml_hygiene_entity_schemas_have_no_commands_key`.

## Acceptance Criteria

- [x] Matrix test covers all cells. Each cell has an explicit assertion, positive or negative. (49 tests = 7 × 7, exceeds the 42-cell minimum by including the attachment row.)
- [x] Snapshot test covers 7 canonical scopes × 2 filter modes = 14 snapshot files. All committed under `swissarmyhammer-kanban/tests/snapshots/`.
- [x] Keybinding tests cover every `keys:` entry on a cross-cutting command (`dd`, `Mod+Backspace`, `Mod+C`, `Mod+X`, `Mod+V`, `Escape`, `Mod+K` plus vim `y`/`x`/`p`/`q`/`:`).
- [x] `yaml_hygiene_entity_schemas_have_no_commands_key` passes.
- [x] `register_commands_returns_expected_count` passes (62 commands, unchanged).
- [x] Zero manual verification steps in the acceptance criteria.

## Tests

- [x] `cargo test -p swissarmyhammer-kanban --test command_surface_matrix` — 49 tests pass.
- [x] `cargo test -p swissarmyhammer-kanban --test command_snapshots` — 14 tests pass.
- [x] `npx vitest run kanban-app/ui/src/lib/keybindings.test.ts` — 59 tests pass.
- [x] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands` — 1396 tests pass.

## Workflow

- Use `/tdd` — write the matrix test skeleton first with `todo!()` or failing assertions, then the prior cards drive each cell green.
- Snapshot tests come last: run the full suite to generate baseline snapshots, review by hand once, commit. After that, any diff means a real regression.

## Regenerating snapshots

Set `UPDATE_SNAPSHOTS=1` to rewrite every snapshot file. Review the diff (`git diff swissarmyhammer-kanban/tests/snapshots/`) before committing. Never edit a snapshot by hand.

```
UPDATE_SNAPSHOTS=1 cargo test -p swissarmyhammer-kanban --test command_snapshots
```

#commands

Depends on: 01KPEN0JMTVSCW8PZW6RRD0NC3 (migration must be complete so snapshots reflect the end state), 01KPEME1897275TKE61EKN6EVX (DeleteProjectCmd retirement)

## Review Findings (2026-04-20 21:10)

All 1396 crate tests + 49 matrix + 14 snapshot + 59 keybindings green. Three-layer test strategy is well-designed and the deliverables match the task description. Inline comments in the matrix are unusually candid about the gap between the card's aspirational "—" cells and the current `Command::available()` contract — good design discipline. Findings below are minor; no blockers.

### Warnings
- [x] `swissarmyhammer-kanban/tests/command_surface_matrix.rs` — `matrix_board_delete_available` / `matrix_board_archive_available` pin a latent bug: the cross-cutting `entity.delete` / `entity.archive` surface on board monikers with `available: true`, but `DeleteEntityCmd` / `ArchiveEntityCmd` have no match arm for `"board"` — dispatching from the surfaced button returns `ExecutionFailed` at runtime. **Resolved (2026-04-20 session 2):** filed follow-up task `01KPPX7FZYZWSN2ECWT0HBGN5H` to add per-type opt-outs in `DeleteEntityCmd::available()` / `ArchiveEntityCmd::available()`. Matrix tests renamed and tightened to pin **both** sides of the current contract: `matrix_board_delete_surfaces_but_dispatch_fails` now dispatches and asserts `CommandError::ExecutionFailed("unknown entity type for delete: 'board'")` so the broken-but-intentional state cannot drift silently. `matrix_board_archive_surfaces_but_should_be_opted_out` keeps the surface pin (archive dispatch through `EntityContext::archive` is generic and does not error at runtime — so a dispatch assertion would over-pin against the fix). Both test comments now reference the follow-up task id.

### Nits
- [x] `swissarmyhammer-kanban/tests/command_surface_matrix.rs` — `matrix_project_copy_not_available` was a misnomer (body asserts `entity.copy` IS available). **Resolved (2026-04-20 session 2):** renamed to `matrix_project_copy_surfaces_despite_card_dash` to match the assertion semantics and the convention used by `matrix_column_copy_available` / `matrix_board_copy_available`.
- [x] Several matrix tests exceed 25s (nextest marks them SLOW). Each test rebuilds a full `KanbanContext` + registers a `StoreHandle` for every entity type + runs `InitBoard`. **Acknowledged, not actioned:** the reviewer flagged this as "Not urgent — 49 tests parallelise under nextest." A shared `OnceCell<Arc<KanbanContext>>` + per-test `TempDir` copy would speed it up, but the integration-level isolation it buys matches what the test is supposed to prove (production-path state mutation per-cell). If CI wall-clock becomes a problem in the future, the harness-pool refactor is a separate performance task and belongs on its own card.
