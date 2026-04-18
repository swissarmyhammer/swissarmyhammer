---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe980
project: spatial-nav
title: Decide whether to keep navOverride (zero production users)
---
## Context

Task `01KPG71NBRXC4JH6CC5CD4XX3N` audited the five deleted `claimWhen` predicate memos (`nameFieldClaimWhen`, `cardClaimPredicates`, `buildCellPredicates`, `edgePredicates`/`predicatesForField`, `buildListClaimPredicates`) and found — by code inspection — that every case they handled is geometric and should fall out of spatial nav's beam test + scoring. No production call sites currently pass a `navOverride`.

An integration test was added in `kanban-app/ui/src/components/focus-scope.test.tsx` (`navOverride end-to-end: broadcastNavCommand routes override target through focus-changed`) that exercises the full `React → spatial_register (overrides) → broadcastNavCommand → spatial_navigate → focus-changed → React focus state` loop. The Rust override selection itself is covered by unit tests in `swissarmyhammer-spatial-nav/src/spatial_state.rs`.

## Decision to make

The `navOverride` mechanism is fully wired end-to-end, tested end-to-end, and unused in production. Three possible paths:

1. **Keep as-is**: leaves a tested escape hatch for future non-geometric navigation (inspector close routing, parent→first-child enter, etc.). Cost: small surface area (one prop, one struct field), zero runtime cost when unused.

2. **Delete**: reduces API surface. Cost: we re-introduce it the first time spatial nav gets a case wrong (parent-to-first-child, wrap-around, modal-exit-to-prev-focus). The integration test still has value as regression protection for `spatial_register`'s `overrides` field, but becomes inert.

3. **Gate on real usage**: delete only if a manual regression pass over the six deleted predicate scenarios produces no divergence from spatial nav. Requires running the app and walking each deleted predicate's scenario by hand.

## Decision

**Option 1: Keep as-is.**

### Rationale

- **Zero runtime cost when unused.** `navOverride` is an optional prop (`navOverride?: Record<string, string | null>`); when omitted it passes `null` through to `spatial_register` and the Rust `overrides` field stays empty. No call sites pay for it today.
- **Small, bounded API surface.** One optional prop on `FocusScope`, one optional parameter on `spatial_register`, one field on `SpatialEntry`. Deletable in one focused patch if the surface ever grows harmful — it doesn't right now.
- **Tested escape hatch has asymmetric value.** Spatial nav is new and will get edge cases wrong (parent→first-child enter, modal-exit-to-prev-focus, inspector close routing were the originating scenarios). Deleting `navOverride` means re-introducing, re-testing, and re-wiring it the first time we hit a non-geometric case — net-negative trade.
- **Production call-site audit confirmed clean.** `rg 'navOverride='` in `kanban-app/ui/src` returns only two matches, both in `focus-scope.test.tsx`. No production consumer exists today; removal pressure is purely aesthetic.
- **Existing tests keep it honest.** The end-to-end integration test in `focus-scope.test.tsx` plus the Rust unit tests in `spatial_state.rs` prevent bit-rot of the override path while it sits idle.

### Option 2 (Delete) — rejected

Removes ~1 prop, ~1 parameter, ~1 field, and a handful of test lines. In exchange we take on the risk of re-implementing the same mechanism — probably less cleanly — the first time spatial nav misroutes a non-geometric case. The current code is already reviewed, tested, and has zero blast radius when unused. Deleting it optimizes for a code-aesthetics metric at the cost of future flexibility.

### Option 3 (Gate on real usage) — rejected

Requires a manual regression pass against a running app, which is not feasible from this environment. Even if performed, it would only de-risk option 2; it does not change the cost/benefit analysis above.

### Manual regression pass

Skipped: requires human validation against a running app, which is outside what this agent can perform. The decision does not depend on the outcome of the manual pass — option 1 is correct under either result (no divergence → keep the mechanism dormant but available; divergence found → keep the mechanism and start using it).

## Acceptance Criteria

- [x] Manual regression pass completed over the five deleted predicate sites with documented outcomes (or a decision to skip that step) — **Skipped: requires human validation against a running app. Decision does not depend on this step.**
- [x] Decision recorded: **keep** (see Decision section above)
- [ ] If delete: remove `navOverride` prop from `FocusScope`, remove `overrides` parameter from `spatial_register`, remove `overrides` field from `SpatialEntry` (Rust), collapse override-specific code paths in `navigate()`, remove both integration and unit tests that target the override path — **Not applicable: decision is keep.**

## References

- Originating card: `01KNQY1GQ9ABHEPE0NJW909EAQ` (introduced `navOverride`)
- Integration test: `kanban-app/ui/src/components/focus-scope.test.tsx`
- Rust override logic: `swissarmyhammer-spatial-nav/src/spatial_state.rs`
- Predicate audit from: `01KPG71NBRXC4JH6CC5CD4XX3N`
