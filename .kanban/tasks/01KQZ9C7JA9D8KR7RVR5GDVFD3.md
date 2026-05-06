---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
position_column: todo
position_ordinal: ea80
project: spatial-nav
title: 'motion-validation: nav.left — production-app spatial test'
---
## What

Add a focused, production-app-mounted spatial test file dedicated to **`nav.left`** that pins the cardinal "left" beam behavior. Family 2 in `spatial-nav-end-to-end.spatial.test.tsx` has one ArrowLeft case; this file exercises the full surface.

Bindings exercised: `h` (vim), `ArrowLeft` (cua), `Shift+Tab` (cua reverse-tab), `Ctrl+b` / `Mod+b` (emacs) — dispatched via `NAV_COMMAND_SPEC` in `kanban-app/ui/src/components/app-shell.tsx:246-250` (`direction: "left"`). Closure awaits `actions.navigate(focusedFq, "left")` → `spatial_navigate` Tauri command.

Kernel behavior under test: `BeamNavStrategy::next` for `Direction::Left` in `swissarmyhammer-focus/src/navigate.rs` — strict half-plane (`cand.right <= from.left`), in-beam vertical-overlap bias, Android beam score `13 * major² + minor²`, leaves-over-containers tie-break.

### File to create

`kanban-app/ui/src/spatial-nav-left.spatial.test.tsx` — same harness pattern as the up/down validation tasks and `spatial-nav-end-to-end.spatial.test.tsx`. Use the 1400×900 Tailwind-substitute stylesheet so columns lay out side-by-side (the same layout fixture `board-view.cross-column-nav.spatial.test.tsx` uses).

### Scenarios (one `it()` each)

- [ ] **cross-column** — focused on `task:D1` (top card in column DOING); ArrowLeft lands on a card in column TODO. Mirror of the existing Family 2 case but pinned to a fresh fixture.
- [ ] **edge stay-put** — focused on a card in the leftmost column; ArrowLeft returns focused FQM. Assert the focused element is unchanged after keydown.
- [ ] **nav-rail to board** — regression for the in-band hard-filter bug fixed in card `01KQZ7VR7JK1QD5QJDDKB529JG`. Focus a leaf in the nav rail; press ArrowRight (the `right` task pins this); then from the landing scope press ArrowLeft and assert it returns to a nav-rail leaf, not stays-put.
- [ ] **layer boundary** — open the inspector on a card; focus a field; ArrowLeft moves to a sibling field on the left within the inspector layer (or stays put if leftmost). Never exits to the board.
- [ ] **Tab reverse parity (CUA)** — cua mode; press `Shift+Tab` from a card; assert the dispatched IPC is `spatial_navigate` with `direction: "left"` and the result matches ArrowLeft from the same starting scope.
- [ ] **vim `h` parity** — vim mode; press `h`; identical result to ArrowLeft.
- [ ] **emacs `Ctrl+b` parity** — emacs mode; press `Ctrl+b`; identical result.

### Out of scope

- Do NOT modify kernel code. If the in-band hard filter is still in place, the **nav-rail to board** scenario stays failing — the kernel-fix card 01KQZ7VR7JK1QD5QJDDKB529JG is what makes it pass.
- Do NOT edit `spatial-nav-end-to-end.spatial.test.tsx`.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/spatial-nav-left.spatial.test.tsx` exists, mounts `App`, mocks Tauri via `@/test/spatial-shadow-registry`.
- [ ] All 7 scenarios above are present as separate `it()` blocks inside one `describe("nav.left — production app", () => { ... })`.
- [ ] Each scenario asserts both the dispatched `spatial_navigate` IPC shape and the post-keystroke `data-focused` element.
- [ ] Passes under `cd kanban-app/ui && bun test spatial-nav-left`.
- [ ] No flake under 5 consecutive runs.

## Tests

- [ ] New file: `kanban-app/ui/src/spatial-nav-left.spatial.test.tsx`.
- [ ] Test command: `cd kanban-app/ui && bun test spatial-nav-left`.
- [ ] Existing spatial tests still pass.

## Workflow

- Use `/tdd` — write each scenario as a failing assertion first.

#motion-validation #stateless-rebuild