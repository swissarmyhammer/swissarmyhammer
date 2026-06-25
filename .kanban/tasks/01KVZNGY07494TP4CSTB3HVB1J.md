---
position_column: todo
position_ordinal: ff8180
title: 'Pre-existing UI browser-suite test failures (~10): editor-save, mention-view extraCommands, entity-card, grid-empty-state, spatial-nav'
---
## What

The `apps/kanban-app/ui` **browser** (Playwright/Chromium) vitest project has ~10 test failures that are PRE-EXISTING on a clean tree — confirmed independently during the `#ui` batch (2026-06-24/25) by git-stashing all working-tree changes and reproducing the failures on `HEAD` with no edits. They are NOT regressions from the command-events or #ui work; the failure set was byte-identical before/after those changes.

Known failing tests (from the last full browser-suite run during `^agk68p1` review — re-confirm exact current set first):
- `apps/kanban-app/ui/src/components/mention-view.test.tsx` — `extraCommands single mode right-click` (a show-context-menu assertion, ~line 567).
- `editor-save` — 6 failing cases (find the file via the suite output; likely `src/components/fields/editors/editor-save.test.tsx`).
- `entity-card` — 1 (find the exact file/case).
- `grid-empty-state` — 1.
- `spatial-nav-end-to-end` — `src/spatial-nav-end-to-end.spatial.test.tsx` — 1.

These are likely INDEPENDENT root causes (different components), so triage each; some may be genuinely flaky vs deterministically red.

NOTE: separate from these assertion failures, the browser project also emits stale `@tauri-apps/api` import/module-resolution errors — those are tracked in their own task; rule them out (clear `node_modules/.vite` / reinstall) before triaging the assertion failures, since a broken dep-optimize cache can cascade into spurious failures.

## Acceptance Criteria
- [ ] Re-run the browser suite on a clean tree and record the EXACT current failing test list (names + files + error messages) — the list above is from an earlier run and must be re-confirmed.
- [ ] Each failing test is either fixed (passes deterministically) OR, if genuinely environment/flake-bound and out of scope to fix now, quarantined with an explicit justification + a linked follow-up — no silent skips.
- [ ] `cd apps/kanban-app/ui && npx vitest run --project browser` reports 0 unexpected failures (a green run, or a documented known-failing allowlist).

## Tests
- [ ] Repro: `cd apps/kanban-app/ui && npx vitest run --project browser 2>&1 | tail -60` — capture the failing set.
- [ ] Per-file: `cd apps/kanban-app/ui && npx vitest run <file> --project browser` for each named file — passes after the fix.

## Workflow
- Triage first (capture the real current failure list), then fix per root cause. Use `/tdd` only where you change behavior to fix an assertion; for flakes, stabilize the test (real awaits, not sleeps) rather than loosening the assertion. #test-failure