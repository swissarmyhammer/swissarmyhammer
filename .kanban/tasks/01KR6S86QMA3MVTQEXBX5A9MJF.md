---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffca80
title: 'Flaky: filter-editor delete-scenario "delete to empty: saved filter clears"'
---
## What

`src/components/filter-editor.delete-scenario.test.tsx > FilterEditor type → type → delete scenario > tag → append tag → delete to empty: saved filter clears` is intermittently failing in browser-mode (chromium) when running the full vitest suite via `pnpm test`.

## Symptoms

- **Isolation**: passes 100% of the time when run alone (`pnpm vitest run src/components/filter-editor.delete-scenario.test.tsx` — 5/5 runs green, 7/7 tests pass each).
- **Full suite**: failed 2 out of 6 full-suite runs during `/loop`-style verification. Latest 4 runs after small file edits all passed.
- The expectation `expect(view.state.doc.toString()).toBe("")` fails with intermediate-typing residue:
  - Once seen as `'#BLOCK'` (line 174 of test in older numbering)
  - Once seen as `'#BLO'`

## Hypothesis

The test types `#BLOCKED` (8 chars) then sends 8 `{Backspace}` events, each via `userEvent.type(view.contentDOM, "{Backspace}")`. The race appears to be: a previous test in the same browser shard occasionally leaves CodeMirror's contentDOM in a state where some of the `#BLOCKED` typing keystrokes are dropped (so the buffer holds e.g. `#BLOCK` instead of `#BLOCKED`), then the 8 backspaces overshoot — they delete what's there plus into "empty" but the doc string check sees the post-overshoot residue.

The `afterEach(cleanup)` hook in the file already addresses one round of cross-test residue (see comment at lines 116-122 referencing task `01KQZ9R9TQF1EQ32MH1NXHEGEN`), but it does not cover all keystroke-loss races.

## What I tried

- Confirmed reliable pass in isolation (5 consecutive runs).
- Confirmed flake in full suite (2 fails / 6 runs).
- Removing 5 unrelated `it.skip` placeholders in `column-view.virtualized-nav.browser.test.tsx` and `spatial-nav-soak.spatial.test.tsx` (kanban hygiene per `/test`) coincided with 4/4 subsequent green runs — but n=4 is not statistically conclusive, this could be noise.
- Did NOT modify the filter-editor or this test file.

## Acceptance criteria

- The full `pnpm test` suite passes 10 consecutive runs without this test failing.
- Either the test is made deterministic (e.g. assert pre-condition `view.state.doc.toString() === "#BLOCKED"` before the backspace loop, then drive backspaces to a buffer-length target rather than a fixed count of 8) OR the keystroke-loss race is fixed in `userEvent`/CodeMirror harness wiring.

## Files

- `kanban-app/ui/src/components/filter-editor.delete-scenario.test.tsx` (test, lines ~163-191)
- `kanban-app/ui/src/components/filter-editor.tsx` (subject under test)

## Tests

- Reproduce: `cd kanban-app/ui && for i in 1 2 3 4 5 6 7 8 9 10; do pnpm test 2>&1 | grep -aE "Tests +|FAIL "; done`
- Once fixed, the same loop must show 10/10 green. #test-failure