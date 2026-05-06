---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
position_column: todo
position_ordinal: ed80
project: spatial-nav
title: 'motion-validation: nav.last — production-app spatial test'
---
## What

Add a focused, production-app-mounted spatial test file dedicated to **`nav.last`** (Direction::Last) that pins the "go to last child / last sibling" behavior. No targeted family in the umbrella end-to-end test exists.

Bindings exercised: `Shift+G` (vim), `End` (cua), `Alt+>` (emacs) — dispatched via `NAV_COMMAND_SPEC` in `kanban-app/ui/src/components/app-shell.tsx:264-268` (`direction: "last"`). Closure awaits `actions.navigate(focusedFq, "last")` → `spatial_navigate` Tauri command.

Kernel behavior under test: `BeamNavStrategy::next` for `Direction::Last` in `swissarmyhammer-focus/src/navigate.rs` and shared `last_child_by_bottom_right` helper in `swissarmyhammer-focus/src/registry.rs`. Per the module docs, **Last focuses children of the focused scope's parent_zone** (siblings, vim G semantics) when the focused scope has a parent, falling back to children-of-self at the layer root. Mirror semantics of nav.first.

### File to create

`kanban-app/ui/src/spatial-nav-last.spatial.test.tsx` — same harness pattern as the cardinal validation tasks.

### Scenarios (one `it()` each)

- [ ] **leaf with siblings** — focused on `task:T1` (top card in column TODO); press End (or `Shift+G` in vim, or `Alt+>` in emacs); lands on `task:T3` (bottommost-then-rightmost sibling under the same parent column).
- [ ] **already-last stay-put** — focused on `task:T3` (bottom card); press End; result equals focused FQM. Assert no `data-focused` change.
- [ ] **layer-root fallback** — focus the layer-root scope; press End; lands on the bottommost-rightmost child of that scope.
- [ ] **column zone → last card** — focus the column header scope (`board:column:TODO`); press End; lands on the bottommost-rightmost child of *its* parent (last column of the board), not into its own children.
- [ ] **vim `Shift+G` parity** — vim mode; press `Shift+G`; identical dispatch to End.
- [ ] **emacs `Alt+>` parity** — emacs mode; press `Alt+>`; identical dispatch to End.
- [ ] **deprecated RowEnd alias** — assert `Direction::RowEnd` (the deprecated alias) still routes through the same path as `Direction::Last`. Implementation note: this assertion can be at the kernel level via a Rust unit test in `swissarmyhammer-focus/src/navigate.rs` if it does not already exist (`deprecated_row_start_end_still_alias_first_last` at line 1090 already covers this — verify and document the link).

### Out of scope

- Do NOT modify kernel code.
- Do NOT edit `spatial-nav-end-to-end.spatial.test.tsx`.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/spatial-nav-last.spatial.test.tsx` exists, mounts `App`, mocks Tauri via `@/test/spatial-shadow-registry`.
- [ ] All 7 scenarios above are present as separate `it()` blocks inside one `describe("nav.last — production app", () => { ... })`.
- [ ] Each scenario asserts both the dispatched `spatial_navigate` IPC shape and the post-keystroke `data-focused` element.
- [ ] Passes under `cd kanban-app/ui && bun test spatial-nav-last`.
- [ ] No flake under 5 consecutive runs.

## Tests

- [ ] New file: `kanban-app/ui/src/spatial-nav-last.spatial.test.tsx`.
- [ ] Test command: `cd kanban-app/ui && bun test spatial-nav-last`.
- [ ] Existing spatial tests still pass.
- [ ] Existing Rust kernel test `deprecated_row_start_end_still_alias_first_last` still passes (`cargo test -p swissarmyhammer-focus`).

## Workflow

- Use `/tdd` — write each scenario as a failing assertion first.

#motion-validation #stateless-rebuild