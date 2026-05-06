---
assignees:
- claude-code
position_column: todo
position_ordinal: f280
project: spatial-nav
title: Rename showFocusBar → showFocus in Rust doc-comments and assert focus-indicator coverage on perspective tabs + nav buttons
---
## What

Two-part cleanup tied to `#stateless-nav`:

### Part A — Rename stale `showFocusBar` references to `showFocus` in Rust comments

The React prop has been called `showFocus` for a while (see `kanban-app/ui/src/components/focus-scope.tsx` — `showFocus?: boolean` on `FocusScopeOwnProps`). Five Rust doc-comment / test-comment sites still reference the old name and read as a contract drift to anyone tracing back from the kernel:

- `swissarmyhammer-focus/src/navigate.rs:68` — module doc-comment: ``showFocusBar=false` container``
- `swissarmyhammer-focus/src/navigate.rs:391` — `better_candidate` doc-comment: ``showFocusBar=false` container``
- `swissarmyhammer-focus/src/navigate.rs:796` — `tied_distances_leaf_wins_over_container` test doc-comment: ``showFocusBar=false` container``
- `swissarmyhammer-focus/tests/perspective_bar_arrow_nav.rs:138` — module doc-comment: `(a `showFocusBar={false}` zone)`
- `swissarmyhammer-focus/tests/perspective_bar_arrow_nav.rs:169` — inline comment: `// showFocusBar=false zone (the bug)`

Replace each `showFocusBar` token with `showFocus` (preserve surrounding `=false` / `={false}` syntax exactly — no other prose changes). No production code paths touched; this is comment-only.

### Part B — Pin focus-indicator coverage on perspective tabs and navigation buttons/tabs

Audit and harden the existing browser-mode focus-indicator tests so a regression that drops `<FocusIndicator>` from a focused perspective tab or nav-bar button trips a test.

Files:
- `kanban-app/ui/src/components/nav-bar.focus-indicator.browser.test.tsx` — already covers four leaves (board-selector, inspect, search, percent-complete field) + the inspect remount race. Verify each case asserts BOTH `data-focused="true"` AND that a `[data-testid="focus-indicator"]` descendant is rendered.
- `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx` — already covers inactive tab focused, active tab focused, focus persistence through activation, and rename commit/cancel. Verify each case asserts both halves of the wiring.

If any case is missing the indicator-descendant assertion, add `expect(wrapper.querySelector('[data-testid="focus-indicator"]')).not.toBeNull()` (or the equivalent waitFor) so the test flips red on a `showFocus` regression.

No new components, no behavior changes — this task only tightens existing tests and renames stale comments.

## Acceptance Criteria
- [ ] Zero matches for `showFocusBar` in `swissarmyhammer-focus/` after the change (`grep -r showFocusBar swissarmyhammer-focus/ | wc -l` returns 0)
- [ ] Each navbar focus-indicator browser test asserts both `data-focused="true"` on the leaf wrapper AND that a `[data-testid="focus-indicator"]` descendant exists
- [ ] Each perspective-tab focus-indicator browser test asserts both halves of the wiring (same as above)
- [ ] No production source files (`*.tsx`, `*.rs` outside doc-comments) modified — this is comments + tests only
- [ ] `cargo build -p swissarmyhammer-focus` passes (no doc-link breakage from the rename)

## Tests
- [ ] Run `cd kanban-app/ui && pnpm vitest run src/components/nav-bar.focus-indicator.browser.test.tsx src/components/perspective-tab-bar.focus-indicator.browser.test.tsx` — all cases pass after tightening
- [ ] Run `cargo test -p swissarmyhammer-focus --test perspective_bar_arrow_nav` — passes (comment-only changes must not affect tests)
- [ ] `grep -rn showFocusBar swissarmyhammer-focus/ kanban-app/` returns no matches

## Workflow
- Use `/tdd` — for each focus-indicator test that lacks the descendant assertion, add it first (expecting red because… actually the assertion should already pass given current production code; treat any unexpected red as a real bug to investigate before adjusting the test).

#stateless-nav