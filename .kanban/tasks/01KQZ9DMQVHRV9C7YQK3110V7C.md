---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
position_column: todo
position_ordinal: eb80
project: spatial-nav
title: 'motion-validation: nav.right — production-app spatial test'
---
## What

Add a focused, production-app-mounted spatial test file dedicated to **`nav.right`** that pins the cardinal "right" beam behavior. Family 2 in `spatial-nav-end-to-end.spatial.test.tsx` has one ArrowRight case (regression for `01KQ7GWE9V2XKWYAQ0HCPDE0EZ`); this file goes wider.

Bindings exercised: `l` (vim), `ArrowRight` (cua), `Tab` (cua forward-tab), `Ctrl+f` / `Mod+f` (emacs) — dispatched via `NAV_COMMAND_SPEC` in `kanban-app/ui/src/components/app-shell.tsx:252-256` (`direction: "right"`). Closure awaits `actions.navigate(focusedFq, "right")` → `spatial_navigate` Tauri command.

Kernel behavior under test: `BeamNavStrategy::next` for `Direction::Right` in `swissarmyhammer-focus/src/navigate.rs` — strict half-plane (`cand.left >= from.right`), in-beam vertical-overlap bias, Android beam score, leaves-over-containers tie-break.

### File to create

`kanban-app/ui/src/spatial-nav-right.spatial.test.tsx` — same harness pattern as the up/down/left validation tasks. Use the 1400×900 Tailwind-substitute stylesheet so columns lay out side-by-side.

### Scenarios (one `it()` each)

- [ ] **cross-column** — focused on `task:T1` (top card in column TODO); ArrowRight lands on a card in column DOING (mirror of the Family 2 case, fresh fixture).
- [ ] **edge stay-put** — focused on a card in the rightmost column; ArrowRight returns focused FQM. Assert focused element unchanged.
- [ ] **nav-rail to board, beam-search bias** — regression for the in-band hard-filter bug fixed by card `01KQZ7VR7JK1QD5QJDDKB529JG`. Focus a leaf in the nav rail (a thin 28-ish px row). Press ArrowRight. Assert focus moves to a board-area scope, NOT stays-put. Currently the in-band requirement drops every candidate that doesn't share that 28px Y band; the kernel fix replaces the hard filter with a score bias.
- [ ] **layer boundary** — open the inspector on a card; focus a field; ArrowRight moves to a sibling field on the right within the inspector layer (or stays put if rightmost). Never exits to the board.
- [ ] **Tab forward parity (CUA)** — cua mode; press `Tab` from a card; assert dispatched IPC is `spatial_navigate` with `direction: "right"` and the result matches ArrowRight from the same starting scope.
- [ ] **vim `l` parity** — vim mode; press `l`; identical result to ArrowRight.
- [ ] **emacs `Ctrl+f` parity** — emacs mode; press `Ctrl+f`; identical result. (Note: `Mod+f` collides with `app.search` in cua/vim — verify emacs map wins for emacs mode.)

### Out of scope

- Do NOT modify kernel code. The **nav-rail to board** scenario stays failing until the kernel-fix card 01KQZ7VR7JK1QD5QJDDKB529JG lands.
- Do NOT edit `spatial-nav-end-to-end.spatial.test.tsx`.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/spatial-nav-right.spatial.test.tsx` exists, mounts `App`, mocks Tauri via `@/test/spatial-shadow-registry`.
- [ ] All 7 scenarios above are present as separate `it()` blocks inside one `describe("nav.right — production app", () => { ... })`.
- [ ] Each scenario asserts both the dispatched `spatial_navigate` IPC shape and the post-keystroke `data-focused` element.
- [ ] Passes under `cd kanban-app/ui && bun test spatial-nav-right`.
- [ ] No flake under 5 consecutive runs.

## Tests

- [ ] New file: `kanban-app/ui/src/spatial-nav-right.spatial.test.tsx`.
- [ ] Test command: `cd kanban-app/ui && bun test spatial-nav-right`.
- [ ] Existing spatial tests still pass.

## Workflow

- Use `/tdd` — write each scenario as a failing assertion first. The nav-rail-to-board scenario is expected to fail until card 01KQZ7VR7JK1QD5QJDDKB529JG fixes the in-band hard filter; record that as a "regression backstop" comment in the test.

#motion-validation #stateless-rebuild