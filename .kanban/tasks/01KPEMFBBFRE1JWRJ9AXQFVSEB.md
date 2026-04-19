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
position_column: todo
position_ordinal: d680
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

- [ ] Implement matrix test — one `#[tokio::test]` per entity row, table-driven over command columns.
- [ ] Implement snapshot tests for the 7 canonical scopes × 2 filter modes.
- [ ] Extend keybinding tests with cross-cutting cases.
- [ ] Add UIState round-trip tests for mutating commands where the existing tests don't already cover them.
- [ ] Confirm `yaml_hygiene_no_cross_cutting_in_entity_schemas` from card 01KPEM811W5XE6WVHDQVRCZ4B0 is GREEN.

## Acceptance Criteria

- [ ] Matrix test covers all 42 cells (6 entities × 7 commands). Each cell has an explicit assertion, positive or negative.
- [ ] Snapshot test covers 7 canonical scopes × 2 filter modes = 14 snapshot files. All committed.
- [ ] Keybinding tests cover every `keys:` entry on a cross-cutting command (dd, Mod+Backspace, Mod+C, Mod+X, Mod+V, Escape, Mod+K at minimum).
- [ ] `yaml_hygiene_no_cross_cutting_in_entity_schemas` passes.
- [ ] `register_commands_returns_expected_count` passes with the final total.
- [ ] Zero manual verification steps in the acceptance criteria.

## Tests

- [ ] `cargo test -p swissarmyhammer-kanban --test command_surface_matrix` — all cells green.
- [ ] `cargo test -p swissarmyhammer-kanban --test command_snapshots` — all snapshots match committed versions.
- [ ] `bun test kanban-app/ui/src/lib/keybindings.test.ts` — all keybinding cases green.
- [ ] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands` — all tests green.

## Workflow

- Use `/tdd` — write the matrix test skeleton first with `todo!()` or failing assertions, then the prior cards drive each cell green.
- Snapshot tests come last: run the full suite to generate baseline snapshots, review by hand once, commit. After that, any diff means a real regression.

#commands

Depends on: 01KPEN0JMTVSCW8PZW6RRD0NC3 (migration must be complete so snapshots reflect the end state), 01KPEME1897275TKE61EKN6EVX (DeleteProjectCmd retirement)