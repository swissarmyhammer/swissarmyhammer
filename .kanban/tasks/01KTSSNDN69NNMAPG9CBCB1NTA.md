---
assignees:
- claude-code
position_column: todo
position_ordinal: eb80
title: 'Pre-existing vitest browser-mode failures: inspectable.space.browser.test.tsx (4) — Space-inspect dispatch contract'
---
## What

While implementing 01KTQEKP9E8TPQ547BWA5RGWH9 (spatial vitest repair, 2026-06-10), running the non-`spatial`-named importers of the shared test modules surfaced 4 pre-existing failures in `apps/kanban-app/ui/src/components/inspectable.space.browser.test.tsx`:

- space_on_focused_inspectable_dispatches_inspect_with_wrapper_moniker
- space_on_focused_descendant_dispatches_inspect_with_nearest_inspectable_moniker
- space_with_kernel_focus_on_card_dispatches_inspect_and_preventDefaults
- Vim-mode parity — vim_space_with_kernel_focus_on_card_dispatches_inspect

**Proven pre-existing**: the identical 4 failures reproduce with the git-HEAD versions of `src/test/mock-command-list.ts` and `src/test/spatial-shadow-registry.ts` swapped back in (baseline run 2026-06-10, cmd: restore HEAD copies → rerun → same failing set). Card 01KTQEKP9E8TPQ547BWA5RGWH9's changes neither cause nor fix them.

NOT covered by 01KTS1C4EX8W6GZYPAYB1T431K (that card lists only focus-scope.test.tsx ×9 + attachment-display ×1 — same family, likely the same synthetic-focus-changed-does-not-reach-the-store root cause described there).

Repro: `cd apps/kanban-app/ui && npx vitest run src/components/inspectable.space.browser.test.tsx`

## Acceptance Criteria
- [ ] Diagnose whether the failures share 01KTS1C4EX8W6GZYPAYB1T431K's root cause (consider folding into that card's fix); all 4 tests green in browser mode without weakening the Space-binding shadow contract the file pins.

## Workflow
- `/tdd` for any production fix. #bug #tests