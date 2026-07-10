---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv6fxebra2m324sc7sdwq1w4
  text: |-
    Picked up. Research findings:

    The card's "sibling files" list is partly stale — the inline harness has already been MOSTLY extracted across the codebase. Two pre-existing shared seams exist:
    - test-helpers/kernel-simulator.ts (installKernelSimulator) — ~8 files already delegate to it. Per GUARDRAIL #2 I am NOT migrating onto this; it's out of scope.
    - test/spatial-shadow-registry.ts (setupSpatialHarness) — full e2e harness, ~10 files use it.

    The genuinely-duplicated INLINE harness (this card's scope) is the spatial focus-projection block — byte-identical across these Group A files:
    - entity-inspector.field-enter-drill.browser.test.tsx (fullest: UNHANDLED, handleSpatialCommand, handleEntityCommand, filterIpcCalls, drillInResponses)
    - board-view.enter-drill-in.browser.test.tsx
    - inspectable.space.browser.test.tsx
    - app-shell.test.tsx
    - perspective-tab-bar.add-enter / .filter-enter
    - nav-bar.search-enter / .inspect-enter
    - left-nav.view-enter, entity-card.inspect-enter, column-view.add-task-enter, board-selector.tear-off-enter, pressable
    - data-table.row-label-focus (DIVERGENT: stores fq->segment, no drill — will leave local or parameterize carefully)
    - command-button.spatial (minimal: register/unregister only)

    The shared core is: spatial_register_scope / spatial_unregister_scope / spatial_focus / spatial_clear_focus over monikerToKey + currentFocusKey + listeners, plus optional drill echo with per-test drillInResponses. The per-file ENTITY/UI-state IPC answers DIVERGE (different entity lists, schemas, keymap mode) — those stay local; only the spatial block extracts. Found a real copy-paste artifact in several files: `if (cmd === "spatial_register_scope" || cmd === "spatial_register_scope")` (dead duplicate OR).

    Plan: create test/mock-spatial-kernel.ts exporting UNHANDLED + a makeSpatialKernelMock() factory (state bundle + handleSpatialCommand parameterized by listeners + optional drillInResponses) + filterIpcCalls helper. Migrate Group A files, keeping their local entity answers. Baseline test counts next.
  timestamp: 2026-06-15T20:35:06.232962+00:00
- actor: claude-code
  id: 01kv6gw950c517djbsq009swk9
  text: |-
    DONE — consolidation complete, moving to review.

    NEW shared module (per GUARDRAIL #2, kept SEPARATE from test-helpers/kernel-simulator.ts):
    - apps/kanban-app/ui/src/test/mock-spatial-kernel.ts — exports UNHANDLED sentinel + makeSpatialKernelMock({ emit, windowLabel? }) returning { monikerToKey, currentFocusKey, drillInResponses, handleSpatialCommand, reset }, plus emitToListenerMap (Map<string,cb[]> container) and emitToCallbackRecord (Record<string,cb> container) emit adapters. Single source of truth for the spatial-kernel echo contract (register/unregister/focus/clear-focus/drill-in/drill-out/navigate).
    - apps/kanban-app/ui/src/test/mock-spatial-kernel.node.test.ts — 7 TDD unit tests pinning the contract (register projection, queued focus emit, idempotent clear, drill echo + drillInResponses override, UNHANDLED fallthrough, reset).

    13 consumer files migrated (inline harness removed, now import the shared module): entity-inspector.field-enter-drill.browser, board-view.enter-drill-in.browser, inspectable.space.browser, app-shell, perspective-tab-bar.add-enter, perspective-tab-bar.filter-enter, nav-bar.search-enter, nav-bar.inspect-enter, left-nav.view-enter, entity-card.inspect-enter, column-view.add-task-enter, board-selector.tear-off-enter, pressable. The only file still containing a `function handleSpatialCommand` definition is the shared module.

    LEFT LOCAL on purpose (GUARDRAIL #1 — divergent contracts, NOT collapsed):
    - command-button.spatial.test.tsx — minimal variant: register/unregister only, returns Promise.resolve(null), uses listenCallbacks object; no focus/drill. Forcing it onto the shared handler would change return values.
    - data-table.row-label-focus.spatial.test.tsx — DIVERGENT: stores fq->segment (reversed) and its TESTS read fqToSegment.entries() directly; the shared map is segment->fq. Unifying would break those assertions.

    Incidental cleanup: removed the dead duplicate-OR artifact `if (cmd === "spatial_register_scope" || cmd === "spatial_register_scope")` that was copy-pasted into most of the inline copies.

    VERIFICATION (fresh):
    - npx tsc --noEmit: exit 0, clean.
    - npx vitest run over all 15 baseline files + new module test: 16 files passed, 76 tests passed, 0 failed.

    Per-file before/after (all unchanged; +7 from the new module test):
    | file | before | after |
    | entity-inspector.field-enter-drill | 6 | 6 |
    | board-view.enter-drill-in | 9 | 9 |
    | inspectable.space | 12 | 12 |
    | app-shell | 17 | 17 |
    | perspective-tab-bar.add-enter | 1 | 1 |
    | perspective-tab-bar.filter-enter | 2 | 2 |
    | nav-bar.search-enter | 1 | 1 |
    | nav-bar.inspect-enter | 1 | 1 |
    | left-nav.view-enter | 1 | 1 |
    | entity-card.inspect-enter | 2 | 2 |
    | column-view.add-task-enter | 2 | 2 |
    | board-selector.tear-off-enter | 1 | 1 |
    | pressable | 7 | 7 |
    | command-button.spatial (left local) | 2 | 2 |
    | data-table.row-label-focus (left local) | 5 | 5 |
    | mock-spatial-kernel.node.test (NEW) | — | 7 |
    Total 69 -> 76 (delta = the 7 new unit tests). No behavior change to the mock contract.

    NOTE on working tree: 3 unrelated files (perspective-tab-bar.tsx, left-nav.view-switch.browser.test.tsx, perspective-tab-bar.activate-and-rename.spatial.test.tsx) show as modified but are parallel-shell work — they do NOT reference the new module and were not touched by this refactor.
  timestamp: 2026-06-15T20:51:56.704389+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb780
project: builtin-commands
title: Extract duplicated mock-spatial-kernel harness into shared test utility across spatial browser tests
---
## Problem

The spatial-kernel / entity-command mock test harness is duplicated near-verbatim across 5+ sibling browser/test files in `apps/kanban-app/ui/src/`. A review of `entity-inspector.field-enter-drill.browser.test.tsx` flagged this (review findings 2026-06-15 11:14) but the extraction is cross-file and out of scope for that bug-fix card (tzav13s). This card tracks the consolidation.

## Duplicated scaffolding

The following pieces are copy-pasted (per `similar` probe, 0.96–1.00 similarity) across files:

- `UNHANDLED` sentinel `Symbol("unhandled")` — distinguishes "handler matched, returned undefined" from "handler doesn't apply, keep looking".
- `handleEntityCommand(command, commandArgs)` — answers entity/schema/ui-state/undo/list-commands/dispatch IPCs (and its `SCHEMAS` dependency).
- `handleSpatialCommand(command, commandArgs)` — the spatial-kernel echo contract (`spatial_drill_in` / `spatial_drill_out` / `spatial_navigate` / `spatial_register_scope` / `spatial_unregister_scope` / `spatial_focus` / `spatial_clear_focus`) including the no-silent-dropout / kernel-echo behavior.
- Supporting maps/state: `drillInResponses`, `monikerToKey`, `currentFocusKey`, `listeners`, plus the `filterIpcCalls(cmd, op)` IPC-call filter helper and the listener/mock setup.

## Sibling files carrying near-verbatim copies

- `entity-inspector.field-enter-drill.browser.test.tsx` (source of the findings)
- `board-view.enter-drill-in.browser.test.tsx`
- `inspectable.space.browser.test.tsx`
- `entity-inspector.spatial-nav.test.tsx`
- `field.spatial-nav.test.tsx`
- `inspector-field.space-inspect.browser.test.tsx`
- `entity-inspector.test.tsx`
- `app-shell.test.tsx`
- `perspective-tab-bar.filter-enter.spatial.test.tsx`
- and the related `spatial-shadow-registry.ts`

(Confirm exact set by `similar`/grep before extracting — the list above is the union flagged by the probes.)

## Target

Extract the shared harness into a canonical module, e.g.:

- `apps/kanban-app/ui/src/test/mock-spatial-kernel.ts` — exporting `UNHANDLED`, `handleSpatialCommand`, the supporting maps, `filterIpcCalls`, and the listener/mock setup.
- Optionally split entity-side into `apps/kanban-app/ui/src/test/entity-command-mock.ts` (`handleEntityCommand` + `SCHEMAS`).

Then import and reuse across all the spatial test files, removing the per-file copies. Single source of truth so the kernel-echo contract change propagates everywhere.

## Acceptance Criteria
- [ ] Shared module(s) under `apps/kanban-app/ui/src/test/` own the harness; no near-verbatim duplicate copies remain in the listed test files.
- [ ] All affected spatial browser/test files import from the shared module(s).
- [ ] Full affected test suite stays green after extraction (`npx vitest run` for the touched files).
- [ ] No behavior change to the mock contract — pure consolidation.

## Notes
Deferred from card tzav13s (Fix 2 failing field-pill spatial-nav browser tests). That card's scope was strictly the 2 failing tests + 3 nit param renames, all done. This is the genuinely-separate cross-file refactor. #frontend #refactor #test