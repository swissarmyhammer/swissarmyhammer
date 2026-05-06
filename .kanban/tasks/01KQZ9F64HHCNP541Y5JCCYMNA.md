---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
position_column: todo
position_ordinal: ec80
project: spatial-nav
title: 'motion-validation: nav.first — production-app spatial test'
---
## What

Add a focused, production-app-mounted spatial test file dedicated to **`nav.first`** (Direction::First) that pins the "go to first child / first sibling" behavior. No targeted family in the umbrella end-to-end test exists for this command.

Bindings exercised: `gg` (vim sequence), `Home` (cua), `Alt+<` (emacs) — dispatched via `NAV_COMMAND_SPEC` in `kanban-app/ui/src/components/app-shell.tsx:258-262` (`direction: "first"`). Closure awaits `actions.navigate(focusedFq, "first")` → `spatial_navigate` Tauri command. The vim sequence is set up in `kanban-app/ui/src/lib/keybindings.ts:88` (`g: { g: "nav.first", ... }`).

Kernel behavior under test: `BeamNavStrategy::next` for `Direction::First` in `swissarmyhammer-focus/src/navigate.rs` and shared `first_child_by_top_left` helper in `swissarmyhammer-focus/src/registry.rs`. Per the module docs, **First focuses children of the focused scope's parent_zone** (siblings, vim G/gg semantics) when the focused scope has a parent, falling back to children-of-self at the layer root. The fix is in HEAD (`navigate.rs:471-493` after `d0460d061`) per card `01KQZ7VR7JK1QD5QJDDKB529JG`.

### File to create

`kanban-app/ui/src/spatial-nav-first.spatial.test.tsx` — same harness pattern as the cardinal validation tasks.

### Scenarios (one `it()` each)

- [ ] **leaf with siblings** — focused on `task:T2` (middle card in column TODO); press Home (or `gg` in vim, or `Alt+<` in emacs); lands on `task:T1` (topmost-then-leftmost sibling under the same parent column).
- [ ] **already-first stay-put** — focused on `task:T1`; press Home; result equals focused FQM. Assert no `data-focused` change.
- [ ] **layer-root fallback** — focus the layer-root scope (e.g., `app:window` or whatever scope has `parent_zone === None`); press Home; lands on the topmost-leftmost child of that scope. Pins the "fallback to children-of-self when parent_zone is None" path.
- [ ] **column zone → first card** — focus the column header scope (`board:column:TODO`); press Home; lands on the topmost-leftmost child of *its* parent (i.e., the first column of the board), not into its own children. This pins the parent-zone-not-self semantics.
- [ ] **vim `gg` sequence** — vim mode; type `g` then `g` within the 500ms sequence window; assert `spatial_navigate` is dispatched with `direction: "first"`. (Use `await new Promise(r => setTimeout(r, 100))` between keys, well under 500ms.)
- [ ] **vim `gg` timeout** — vim mode; type `g`; wait 600ms; type `g`; assert NO `spatial_navigate` call (the second `g` falls outside the sequence window and is treated as a fresh prefix).
- [ ] **emacs `Alt+<` parity** — emacs mode; press `Alt+<`; identical dispatch to Home.

### Out of scope

- Do NOT modify kernel code. The HEAD fix from `01KQZ7VR7JK1QD5QJDDKB529JG` should already make the leaf-with-siblings scenario pass; if it does not, file a sub-bug rather than weakening the assertion.
- Do NOT edit `spatial-nav-end-to-end.spatial.test.tsx`.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/spatial-nav-first.spatial.test.tsx` exists, mounts `App`, mocks Tauri via `@/test/spatial-shadow-registry`.
- [ ] All 7 scenarios above are present as separate `it()` blocks inside one `describe("nav.first — production app", () => { ... })`.
- [ ] Each scenario asserts both the dispatched `spatial_navigate` IPC shape and the post-keystroke `data-focused` element.
- [ ] Passes under `cd kanban-app/ui && bun test spatial-nav-first`.
- [ ] No flake under 5 consecutive runs.

## Tests

- [ ] New file: `kanban-app/ui/src/spatial-nav-first.spatial.test.tsx`.
- [ ] Test command: `cd kanban-app/ui && bun test spatial-nav-first`.
- [ ] Existing spatial tests still pass.

## Workflow

- Use `/tdd` — write each scenario first as a failing assertion. The leaf-with-siblings case may already pass on HEAD (post-`d0460d061`); the test pins it as a regression backstop.

#motion-validation #stateless-rebuild