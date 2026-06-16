---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv88bdrpva9g2qa9tsj8a5ch
  text: 'Finish loop: verified 12/12 browser tests pass + tsc --noEmit clean. Scoped review (single file) found only the already-captured harness duplication (zd74s4t) + pre-existing clarity nits in that same shared harness — no new in-scope findings. Moved to done.'
  timestamp: 2026-06-16T13:01:24.630737+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb980
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
- [x] Diagnose whether the failures share 01KTS1C4EX8W6GZYPAYB1T431K's root cause (consider folding into that card's fix); all 4 tests green in browser mode without weakening the Space-binding shadow contract the file pins.

## Resolution (2026-06-12)

The 4 failures no longer reproduce — they were fixed by the Card G/H consolidation commits that landed AFTER this card was filed:

- e064bcb17 `refactor(commands): single-source entity.inspect + nav.focus; retire view.switch minting (Cards G+H)` — rewrote this test file to pin the NEW contract: Space routes the single GLOBAL plugin-owned `entity.inspect` to the backend (one `dispatch_command` carrying the focused leaf-first scope chain; server-side innermost-inspectable resolution via `resolveInspectTarget` in `builtin/plugins/app-shell-commands/commands/ui.ts`), zero webview-side `app.inspect` synthesis. The same commit added the MCP set-focus echo translation to the file's `makeDefaultInvokeImpl` harness so click-driven focus claims reach the entity-focus store.
- 6dc3b7d07 renamed `ui.inspect` → `app.inspect` (test helpers track the rename).
- b78558fdb (Card J chords) kept the file in sync.

Two of the named tests now exist under their post-Card-G renamed forms:
- space_on_focused_inspectable_dispatches_inspect_with_wrapper_moniker → space_on_focused_inspectable_dispatches_single_backend_entity_inspect
- space_on_focused_descendant_dispatches_inspect_with_nearest_inspectable_moniker → space_on_focused_descendant_dispatches_chain_led_by_nearest_inspectable_moniker

Root cause was NOT 01KTS1C4EX8W6GZYPAYB1T431K's synthetic-focus-changed gap as a remaining bug — the harness-side fix (command_tool_call focus echo translation) was folded into this file during Card G.

**Verification (2026-06-12, no code changes required)**:
- `cd apps/kanban-app/ui && npx vitest run --project browser --reporter verbose src/components/inspectable.space.browser.test.tsx` → 12/12 pass in browser (chromium), including all 4 named tests; re-run confirmed (3 consecutive green runs).
- `npx tsc --noEmit` → exit 0, clean.

## Workflow
- `/tdd` for any production fix. #bug #tests