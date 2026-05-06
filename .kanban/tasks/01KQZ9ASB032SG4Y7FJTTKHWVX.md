---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
position_column: todo
position_ordinal: e980
project: spatial-nav
title: 'motion-validation: nav.down — production-app spatial test'
---
## What

Add a focused, production-app-mounted spatial test file dedicated to **`nav.down`** that pins the cardinal "down" beam behavior. The umbrella `spatial-nav-end-to-end.spatial.test.tsx` Family 2 has one ArrowDown case; this file goes deeper.

Bindings exercised: `j` (vim), `ArrowDown` (cua), `Ctrl+n` / `Mod+n` (emacs) — dispatched via `NAV_COMMAND_SPEC` in `kanban-app/ui/src/components/app-shell.tsx:240-244` (`direction: "down"`). Closure awaits `actions.navigate(focusedFq, "down")` → `spatial_navigate` Tauri command.

Kernel behavior under test: `BeamNavStrategy::next` for `Direction::Down` in `swissarmyhammer-focus/src/navigate.rs` — strict half-plane (`cand.top >= from.bottom`), in-beam horizontal-overlap bias, Android beam score, leaves-over-containers tie-break.

### File to create

`kanban-app/ui/src/spatial-nav-down.spatial.test.tsx` — same harness pattern as the `up` validation task and `spatial-nav-end-to-end.spatial.test.tsx`.

### Scenarios (one `it()` each)

- [ ] **basic vertical sibling** — focused on `task:T1` (top card); ArrowDown lands on `task:T2`.
- [ ] **multi-step descent** — from `task:T1`, press ArrowDown twice; lands on `task:T3`. Pins that successive Downs walk the column.
- [ ] **edge stay-put** — focused on the bottom card of column TODO; ArrowDown returns the focused FQM (no `data-focused` change in the same column; if the kernel escalates to a layer below, the test must assert the kernel returned the focused FQM via the `spatial_navigate` mock result, not the visible element).
- [ ] **cross-zone descent** — focused on the column-header zone (`board:column:TODO`); ArrowDown drills into the column's first card by reading order (top-left). Assert post-keydown focus is `task:T1`.
- [ ] **layer boundary** — open the inspector on a card, focus a field; ArrowDown moves to the next field below within the inspector layer; never exits to the board.
- [ ] **vim `j` parity** — vim mode; press `j`; identical result to ArrowDown.
- [ ] **emacs `Ctrl+n` parity** — emacs mode; press `Ctrl+n`; identical result.

### Out of scope

- Do NOT modify kernel code. If a scenario fails (e.g., column-header → first-card descent isn't yet implemented), the test stays failing and the kernel-fix card (`01KQZ7VR7JK1QD5QJDDKB529JG`) addresses it.
- Do NOT edit `spatial-nav-end-to-end.spatial.test.tsx`.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/spatial-nav-down.spatial.test.tsx` exists, mounts `App`, mocks Tauri via `@/test/spatial-shadow-registry`.
- [ ] All 7 scenarios above are present as separate `it()` blocks inside one `describe("nav.down — production app", () => { ... })`.
- [ ] Each scenario asserts both the dispatched `spatial_navigate` IPC shape and the post-keystroke `data-focused` element.
- [ ] Passes under `cd kanban-app/ui && bun test spatial-nav-down`.
- [ ] No flake under 5 consecutive runs.

## Tests

- [ ] New file: `kanban-app/ui/src/spatial-nav-down.spatial.test.tsx`.
- [ ] Test command: `cd kanban-app/ui && bun test spatial-nav-down`.
- [ ] Existing spatial tests still pass.

## Workflow

- Use `/tdd` — write each scenario first; if a scenario fails on current `main`, leave it failing as a regression backstop and let the kernel-fix card make it pass.

#motion-validation #stateless-rebuild