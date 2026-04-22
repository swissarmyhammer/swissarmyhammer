---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8a80
project: spatial-nav
title: Finish partial registerClaim → registerSpatialKey refactor in entity-focus-context
---
## What

The working tree has an in-progress refactor in `kanban-app/ui/src/lib/entity-focus-context.tsx` that renames the claim-registry API:

- Removed exports: `registerClaim`, `unregisterClaim`, `ClaimCallback`, `useClaimRegistry`
- New exports: `registerSpatialKey`, `unregisterSpatialKey`, `useSpatialKeyRegistry`

The refactor was not propagated to all callers. As a result every browser spatial-nav test crashes at render time with `TypeError: registerClaim is not a function`, originating from `kanban-app/ui/src/components/focus-scope.tsx:103`.

This is pre-existing — discovered while implementing `01KPTFSDB4FKNDJ1X3DBP7ZGNZ` (multi-inspector layer isolation). It blocks all browser spatial-nav test runs in the current working tree, including every existing test file and the new multi-inspector test.

## Affected files

Callers still using the old API (need to be updated):

- `kanban-app/ui/src/components/focus-scope.tsx` — lines 102-111, `useClaimRegistration` destructures `registerClaim`/`unregisterClaim` and passes a `setIsClaimed` callback.
- `kanban-app/ui/src/lib/entity-focus-context.test.tsx` — lines 278, 299, 339, 431, 437-438, 444-445, 500, 504 — unit tests for the old API.
- `kanban-app/ui/src/components/grid-view.test.tsx` — lines 79-80 — mocks the old API.
- `kanban-app/ui/src/components/inspectors-container.test.tsx` — lines 82-83 — mocks the old API.

## Acceptance Criteria

- [x] `focus-scope.tsx` uses the new `registerSpatialKey`/`unregisterSpatialKey` API; the callback-less registration is handled via the focused-moniker store subscription that the refactor enabled (FocusScope subscribes to focused moniker and compares internally instead of receiving a `setIsClaimed` callback).
- [x] Test files updated to reference the new API (or simplified if the old callback-driven shape is no longer needed).
- [x] `cd kanban-app/ui && npm test` — all browser and unit tests green, including the existing `spatial-nav-*.test.tsx` files and the new `spatial-nav-multi-inspector.test.tsx`.

## Tests

- [x] `npx vitest run spatial-nav-inspector` passes
- [x] `npx vitest run spatial-nav-inspector-over-grid` passes
- [x] `npx vitest run spatial-nav-multi-inspector` passes (multi-inspector isolation test from `01KPTFSDB4FKNDJ1X3DBP7ZGNZ`)
- [x] `npx vitest run entity-focus-context.test.tsx` passes

## Context

Discovered while verifying layer isolation end-to-end for inspector nav. The Rust-side layer isolation is fully verified (see `01KPTFSDB4FKNDJ1X3DBP7ZGNZ`) — the new unit tests and parity cases all pass. This card is purely about re-wiring the React layer to match the already-refactored context API.

## Resolution (2026-04-21)

Resolved by downstream work before this card was picked up. A parallel agent (race condition noted in the original filing) propagated the `registerClaim` → `registerSpatialKey` rename to all affected call sites:

- `kanban-app/ui/src/components/focus-scope.tsx`
- `kanban-app/ui/src/lib/entity-focus-context.test.tsx`
- `kanban-app/ui/src/components/grid-view.test.tsx`
- `kanban-app/ui/src/components/inspectors-container.test.tsx`

Verification performed while picking up this card:

- `grep -r registerClaim\|unregisterClaim\|ClaimCallback\|useClaimRegistry kanban-app/ui` — zero matches (production and tests).
- `registerSpatialKey`/`unregisterSpatialKey`/`useSpatialKeyRegistry` present in all five expected files (the four callers above plus the context source itself).
- Targeted acceptance tests: `npx vitest run spatial-nav-inspector spatial-nav-inspector-over-grid spatial-nav-multi-inspector entity-focus-context.test.tsx` — **4 files, 39/39 tests passing**.
- Full UI suite: `cd kanban-app/ui && npm test` — **129 files, 1397/1397 tests passing**.

No code changes were needed in this card.