---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffa380
project: spatial-nav
title: 'repair UI test regression: 18 failing files / 91 failing tests on kanban branch'
---
## Why

The UI test suite has regressed badly. Earlier in the session (after step 12 follow-ups landed) it was 1988 passed / 5 skipped / 0 failed. Now it's 18 failed test files / 91 failed tests. The recent `showFocusBar → showFocus` rename did NOT cause this — verified by stashing the rename and reproducing the same failures on the bare branch.

Step 12's review findings explicitly fixed two classes of issue that have come back:
- `EntityFocusProvider` wrapper missing in test renders (causing `useFocusActions` calls in `FilterEditorDrillOutWiring` to throw)
- `ui:board` chrome zone references in tests (the segment was removed in commit `8232b25cc`)

Something between step 12's followup landing and now regressed these fixes. Possible causes:
- An agent (in this session or another) reverted parts of the step-12 follow-up changes
- A merge / rebase clobbered the fixes
- New test files were added that don't carry the wrappers

## Investigation

1. **Reproduce**: `pnpm -C kanban-app/ui test --run` and capture the 18 failing files.
2. **Categorize**: group failures by root cause. Likely two big buckets:
   - `useFocusActions` outside `EntityFocusProvider` (`FilterEditorDrillOutWiring`) — needs `EntityFocusProvider` in the test stack OR the test needs a different mount root.
   - `ui:board` segment references — tests should use `board:{id}` (entity zone).
   - Possibly more — investigate.
3. **Diff against step-12 fixed state**: look at `git log --oneline kanban-app/ui/src/components/board-view.spatial.test.tsx` and similar — find when these tests were last passing and what changed since.

## Fix

For each failing test file:
- Add `<EntityFocusProvider>` to its test stack if missing.
- Update `ui:board` → `board:{id}` selectors / assertions.
- For any third-cause failures, fix at the test-helper level if shared, otherwise per-file.

Don't sledgehammer-edit production code to make tests pass. The production-side `ui:board` removal and `FilterEditorDrillOutWiring` were deliberate — tests need to catch up.

## Acceptance criteria

- `pnpm -C kanban-app/ui test --run` passes (or only retains genuinely pre-existing skips, no new failures)
- `cargo test -p swissarmyhammer-focus` still green
- No production-side reverts of the `ui:board` removal or `FilterEditorDrillOutWiring`

## Files

Live grep — `cd kanban-app/ui && pnpm exec vitest run 2>&1 | grep -E '✗|FAIL|failed'` to enumerate. Likely starting points:
- `kanban-app/ui/src/components/board-view.spatial.test.tsx`
- `kanban-app/ui/src/components/perspective-tab-bar.test.tsx`
- ~16 others to discover