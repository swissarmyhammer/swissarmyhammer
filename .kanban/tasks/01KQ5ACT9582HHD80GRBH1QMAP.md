---
assignees:
- claude-code
depends_on:
- 01KQ56YQPD6W973XZFGG0GMYWJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffb880
project: spatial-nav
title: Fix FocusScopeBody flex chain breakage at the source (remove outer-flex + inner-flex duplication)
---
## What

`<FocusScope kind="zone">` mounts an inner `FocusScopeBody` chrome div with no `className` (default block-display). This inner div sits between the outer `<FocusZone>` (where consumers' `className` lands) and the children, breaking the flex chain.

Three call sites have now had to work around this individually with the same outer-flex + inner-flex pattern:

1. `kanban-app/ui/src/components/entity-card.tsx` — outer flex on FocusScope, inner flex wrapper on card body
2. `kanban-app/ui/src/components/entity-inspector.tsx` — outer flex on FocusScope, inner `<div className="flex items-start gap-2 w-full">` wrapper around icon + content
3. `kanban-app/ui/src/components/column-view.tsx` — outer FocusScope `position: relative`, inner `<div className="absolute inset-0 flex flex-col min-w-0">` (escape via absolute positioning)

This is duplication of the same root-cause workaround. The next call site that needs flex layout inside a `<FocusScope kind="zone">` will hit the same trap.

## Fix (implemented)

Eliminated `FocusScopeBody` entirely. The chrome (right-click context menu, double-click → inspect, scrollIntoView) is now attached directly to the spatial primitive (`<Focusable>` / `<FocusZone>`). To make `scrollIntoView` reach the primitive's DOM node, both primitives now accept an optional `ref` prop that merges with their internal ResizeObserver ref. The consumer's `className` lands on the same element whose children are direct layout children — no intervening wrapper, no broken flex chain.

After the fix, the call-site workarounds were simplified:
- `entity-inspector.tsx`: removed the inner `<div className="flex items-start gap-2 w-full">` wrapper; icon and content are direct children of the FocusScope's outer primitive.
- `column-view.tsx`: removed the `relative` + `absolute inset-0 flex flex-col` workaround; folded `flex flex-col` into the FocusScope's `className` directly.
- `entity-card.tsx`: already had a clean consumer-managed structure (its own card div for shape + flex). No change needed; verified unchanged behaviour through tests.

## Files

- `kanban-app/ui/src/components/focus-scope.tsx` — removed `FocusScopeBody`; chrome now composes onto the primitive directly
- `kanban-app/ui/src/components/focus-zone.tsx` — added optional `ref` prop
- `kanban-app/ui/src/components/focusable.tsx` — added optional `ref` prop
- `kanban-app/ui/src/components/entity-inspector.tsx` — simplified
- `kanban-app/ui/src/components/column-view.tsx` — simplified
- `kanban-app/ui/src/components/data-table.tsx` — updated stale comment
- `kanban-app/ui/src/components/focus-scope.test.tsx` — added regression test
- `kanban-app/ui/src/components/column-view.test.tsx` — updated regression-guard test for new contract
- `kanban-app/ui/src/components/entity-inspector.test.tsx` — updated stale comment

## Acceptance Criteria

- [x] `FocusScopeBody` no longer breaks the flex chain by default
- [x] Three current workaround call sites simplified — single layer of flex classes, not double
- [x] Existing layout (cards, inspector field rows, column scroll) unchanged visually
- [x] Spatial-nav zone registration / focus / context menu / right-click / scrollIntoView all still work
- [x] `pnpm vitest run` passes (1568/1568 tests pass, 143/143 files pass)
- [x] Added a test that mounts `<FocusScope kind="zone" className="flex flex-row">` inside a SpatialFocusProvider+FocusLayer and asserts children are direct children of the zone div (no inner wrapper)

## Tests

- [x] `focus-scope.test.tsx` — flex parent test: `<FocusScope kind="zone" className="flex flex-row">{a}{b}</FocusScope>` lays children as direct children of the spatial-zone div (regression test added)
- [x] Existing entity-card, entity-inspector, column-view tests still pass after the call-site simplifications
- [x] `focusable.test.tsx`, `focus-zone.test.tsx` — isolated unit tests pinning the merged callback-ref contract for the optional `ref` prop (RefObject and callback-ref forms; covers mount-time forwarding and unmount-time null cleanup; verifies the primitive's internal ref still drives spatial registration).

## Workflow

- Use `/tdd` — write the failing layout test against the spatial stack first, then fix `FocusScopeBody`, then simplify the three call sites.

## Origin

Surfaced during review of `01KQ56YQPD6W973XZFGG0GMYWJ` (Inspector layout regression). Reviewer noted the same workaround appears in three places and recommended fixing the root cause.

This card is "fix the duplication"; the original regression cards (`01KQ56YQPD6W973XZFGG0GMYWJ`, `01KQ4YF38NJ0BN6FM5EVDVRH1N`, plus the entity-card fix in `01KQ20NMRQQSXVRHP4RHE56B0K`) restored functionality at three different layers — this card folds them back to a single fix at the FocusScope layer.

## Review Findings (2026-04-26 12:40)

Implementation is correct: 1564/1564 vitest pass, cargo + clippy clean, tsc clean. The regression test pins the no-inner-wrapper contract for `kind="zone"` FocusScopes. The architectural decision (route chrome onto the primitive directly) is sound — fewer DOM nodes and the consumer's `className` lands where layout actually applies.

### Nits

- [x] `kanban-app/ui/src/components/focus-scope.tsx` — Stale comment references the removed `FocusScopeBody`: "Drives the inner FocusScopeBody's scroll-into-view effect…". The scroll-into-view effect now lives in `FocusScopeChrome`. Update to "Drives the scroll-into-view effect in `FocusScopeChrome`…". **Fixed.**
- [x] `kanban-app/ui/src/components/entity-inspector.test.tsx` — `renderInspectorWithSpatial` docstring rewrites the rationale without referencing the removed `FocusScopeBody`. The helper's purpose is now stated as exercising the production primitive code path (ResizeObserver/click + className-on-primitive contract) that the no-spatial-context fallback `<div>` does not. **Fixed.**
- [x] `kanban-app/ui/src/components/column-view.tsx:886` — Comment says hooks live in the column body so they sit alongside the virtualizer "keeping the outer `<FocusScope>` wrap untouched, which the FocusScopeBody fix in a parallel card also edits." The "parallel card" is this very task. After this lands the reference to a parallel in-flight fix is misleading. Either drop the trailing clause or rephrase to past tense. **DEFERRED to dynamic-lifecycle agent (parallel-safety: this task agent must avoid column-view.tsx).** Suggested rewrite: replace ", which the FocusScopeBody fix in a parallel card also edits" with "." (drop trailing clause) or with " (the FocusScope wrap was rewritten when `FocusScopeBody` was removed)". **Fixed.**
- [x] `kanban-app/ui/src/components/focusable.test.tsx`, `focus-zone.test.tsx` — Added isolated unit tests on each primitive for the new optional `ref` prop. Both `RefObject` and callback-ref forms are exercised; the tests assert the external ref points at the same `<div>` that carries `data-moniker` (i.e. the primitive's internal-ref target), survive mount/unmount, and verify the internal ref still drives `spatial_register_*`. **Fixed.**

### Verification

- `pnpm vitest run` → 1568/1568 pass, 143/143 files pass (up +4 from new ref tests).
- `pnpm tsc --noEmit` → clean.

## Review Findings (2026-04-26 12:55)

Third-pass review. The two checked nits from the prior pass are confirmed fixed in tree:

- `focus-scope.tsx` — the comment near `useIsDirectFocus(moniker)` now reads "Drives the scroll-into-view effect in `FocusScopeChrome`…", matching the relocated effect.
- `entity-inspector.test.tsx` — the `renderInspectorWithSpatial` docstring now describes the production primitive code path (ResizeObserver/click + className-on-primitive contract) without referencing the removed `FocusScopeBody`.
- `focusable.test.tsx`, `focus-zone.test.tsx` — each primitive has two new isolated tests for the optional `ref` prop (RefObject + callback-ref). Both verify the external ref points at the same `<div>` that carries `data-moniker`, that callback-ref cleanup nulls on unmount, and that the primitive's internal ref still drives `spatial_register_focusable` / `spatial_register_zone`. The tests are tightly scoped and don't duplicate the contract already exercised in `focus-scope.test.tsx`.

The remaining `FocusScopeBody` references in `entity-inspector.tsx`, `column-view.test.tsx`, `focus-scope.tsx`, and `focus-scope.test.tsx` are all historical/explanatory documentation in regression-test docstrings and design comments — they describe the pre-fix state to motivate the regression guards. Those are intentional and should stay.

Verification re-run on this branch:

- `pnpm vitest run` → 1571/1571 pass, 143/143 files pass.
- `pnpm tsc --noEmit` → clean.
- `cargo clippy --all-targets --all-features` → clean.

### Outstanding

- [x] `kanban-app/ui/src/components/column-view.tsx:941` — Carryover from prior pass. Comment still reads "keeping the outer `<FocusScope>` wrap untouched, which the FocusScopeBody fix in a parallel card also edits." This task has now landed, so the "parallel card" reference is no longer accurate. The original reviewer deferred this to the dynamic-lifecycle agent for parallel-safety; that justification still applies if the dynamic-lifecycle work is in flight. Leave the carryover open for the dynamic-lifecycle agent (or a follow-up sweep) — do not advance this task to `done` until that comment lands. **Fixed (2026-04-26):** comment rewritten to past-tense — trailing clause now reads "(the FocusScope wrap was rewritten when `FocusScopeBody` was removed)".