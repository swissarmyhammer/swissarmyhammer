---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
position_column: todo
position_ordinal: e880
project: spatial-nav
title: 'motion-validation: nav.up — production-app spatial test'
---
## What

Add a focused, production-app-mounted spatial test file dedicated to **`nav.up`** that pins the cardinal "up" beam behavior across kernel + IPC + React layers. This is more granular than the umbrella `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` (which has one ArrowDown case in Family 2 but no targeted Up coverage).

Bindings exercised: `k` (vim), `ArrowUp` (cua), `Ctrl+p` / `Mod+p` (emacs) — all dispatched via `NAV_COMMAND_SPEC` in `kanban-app/ui/src/components/app-shell.tsx:234-238` (`direction: "up"`). The closure awaits `actions.navigate(focusedFq, "up")` which fires the `spatial_navigate` Tauri command.

Kernel behavior under test: `BeamNavStrategy::next` for `Direction::Up` in `swissarmyhammer-focus/src/navigate.rs` — strict half-plane test (`cand.bottom <= from.top`), in-beam horizontal-overlap bias, Android beam score `13 * major² + minor²`, leaves-over-containers tie-break, no-silent-dropout (return focused FQM at the visual edge).

### File to create

`kanban-app/ui/src/spatial-nav-up.spatial.test.tsx`

Same harness pattern as `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx`: `import App from "@/App"`, hoist Tauri mocks via `@/test/spatial-shadow-registry`, mount under the browser project (Playwright Chromium) with the 1400×900 Tailwind-substitute stylesheet so column geometry is realistic.

### Scenarios (one `it()` each)

- [ ] **basic vertical sibling** — focused on `task:T2` (middle card in column TODO); ArrowUp lands on `task:T1` (above it). Assert via `[data-focused='true'][data-segment]` query and `data-moniker="task:T1"`.
- [ ] **edge stay-put** — focused on `task:T1` (top card); ArrowUp returns focused FQM (no `data-focused` change). Assert by re-reading `[data-focused='true']` after the keydown — same element.
- [ ] **cross-zone escalation** — focused on the top card of column TODO; press ArrowUp, lands on the column header / nav-bar item directly above (whichever scope the production registry has registered above the column). The test asserts the result is in a different zone (`columnOfTaskMoniker(moniker) === null` for the new focus, or it matches a known nav-bar segment).
- [ ] **layer boundary** — open the inspector on a card, focus a field inside; ArrowUp moves within the inspector layer only — the result moniker's `layer_fq` matches the inspector layer (assert via `harness.lastSpatialNavigateCall().layer_fq` if the shadow registry exposes it; otherwise assert the focused element is still inside the inspector DOM subtree).
- [ ] **vim `k` parity** — set keymap mode to vim (via `app-shell` mode); press `k`; assert identical result to ArrowUp from the same starting scope.
- [ ] **emacs `Ctrl+p` parity** — set keymap mode to emacs; press `Ctrl+p`; assert identical result.

### Cross-mode parity is asserted via the shared `spatial_navigate(focusedFq, "up")` IPC

The body of each `it()` after seeding focus should call `mockInvoke.mockClear()` → fire the keydown → assert `spatial_navigate` was invoked with `direction: "up"` and the seeded focused FQ, then assert the post-event `data-focused` element matches the expected moniker (or matches the original moniker for stay-put).

### Out of scope

- Do NOT modify kernel code — this task only adds a test. If a scenario fails because the kernel is wrong, file a separate bug card; do NOT silently weaken the assertion.
- Do NOT edit `spatial-nav-end-to-end.spatial.test.tsx` — leave the umbrella test untouched.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/spatial-nav-up.spatial.test.tsx` exists, imports `App` from `@/App`, mocks the Tauri boundary via `@/test/spatial-shadow-registry`, and runs under the browser-mode vitest project.
- [ ] All 6 scenarios above are present as separate `it()` blocks inside one `describe("nav.up — production app", () => { ... })`.
- [ ] Each scenario asserts both (a) the dispatched `spatial_navigate` call shape and (b) the post-keystroke `data-focused` element.
- [ ] The whole file passes under `cd kanban-app/ui && bun test spatial-nav-up`.
- [ ] No flake under 5 consecutive runs.

## Tests

- [ ] New file: `kanban-app/ui/src/spatial-nav-up.spatial.test.tsx` — content per the scenarios above.
- [ ] Test command: `cd kanban-app/ui && bun test spatial-nav-up` — all scenarios green.
- [ ] Existing `spatial-nav-end-to-end.spatial.test.tsx` still passes (no shared-state collision).

## Workflow

- Use `/tdd` — write each `it()` first as a failing assertion against the production app, then verify the kernel already produces the expected behavior. Where the kernel is wrong (e.g., the cardinal beam in-band hard filter from card 01KQZ7VR7JK1QD5QJDDKB529JG), the test stays failing and the linked card fixes it.

#motion-validation #stateless-rebuild