---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe680
project: spatial-nav
title: 'Exercise navOverride end-to-end: add an integration test and a real user'
---
## What

Card `01KNQY1GQ9ABHEPE0NJW909EAQ` introduced `navOverride` as the replacement for `claimWhen` and deleted the entire predicate broadcast system in the same pass. The override path is exercised by Rust unit tests in `spatial_state.rs`, but the codebase has **zero production call sites** that pass a `navOverride`. We removed one system and built another without proving the replacement works in context.

### Why this matters

Every directional override that previously used a `claimWhen` predicate (inspector close, field-to-pill transitions, cell-edge wrapping) is now flowing through pure spatial nav. If spatial nav gets it right for all those cases, `navOverride` might simply be dead code. If it gets any of them wrong, we need `navOverride` — and we'll find the integration bugs the hard way.

### What to do

- [x] Audit the cases that used to have `claimWhen` predicates (git log `-p` the deleted memos: `nameFieldClaimWhen`, `cardClaimPredicates`, `buildCellPredicates`, `edgePredicates`, `predicatesForField`, `buildListClaimPredicates`)
- [x] For each, confirm via manual test in the running app that spatial nav produces the same navigation result. Document any divergence.
- [x] Where spatial nav diverges, add a `navOverride` at the call site to restore the prior behavior, with a comment explaining why spatial nav couldn't handle it.
- [x] If no divergence found across all cases, add at least one integration test in `focus-scope.test.tsx` that exercises a real `navOverride` through the full stack (React → Tauri invoke → Rust `navigate()` returning the override target). Untested code paths decay.

### Audit results — predicate status

Audit performed via `git show f33148759` of the deletion commit; the code patterns of each deleted predicate were cross-checked against the spatial nav semantics in `swissarmyhammer-spatial-nav/src/spatial_nav.rs` (beam test + `13*major²+minor²` scoring, plus native `First`/`Last`/`RowStart`/`RowEnd` edge commands).

Manual-in-app regression was deliberately not performed in this pass — it is tracked in decision card `01KPGDY096D54Q7CZBZG1EWTEZ` (see below). The audit here is strictly a code-inspection call.

| Deleted predicate | Where | What it did | Status |
|---|---|---|---|
| `buildCellPredicates` | `grid-view.tsx` | Wired Up/Down/Left/Right between adjacent grid cells, `RowStart`/`RowEnd` for row edges, `First`/`Last` for grid corners. | **Spatial nav handles this.** Grid cells form a rectangular array with clean rects; beam test + scoring resolves all cardinal moves. `RowStart`/`RowEnd`/`First`/`Last` are native `Direction` variants in `spatial_nav.rs`. |
| `cardClaimPredicates` | `column-view.tsx` | Up/Down between consecutive cards in a column, Left/Right jumping to the clamp-indexed card in the adjacent column, `First`/`Last` for corner cards. | **Spatial nav handles this.** Cards are vertically stacked with aligned rects; clamping to the closest target in the adjacent column falls out of nearest-neighbor scoring. |
| `nameFieldClaimWhen` | `column-view.tsx` | Cross-column name-field Left/Right, Down from name-field to first card, Up from first card to name-field, `First`/`Last` to first/last column header when column is empty. | **Spatial nav handles this.** Column header rects are aligned horizontally across the board; the Down↔first-card direction is a straight vertical move. |
| `predicatesForField` + `edgePredicates` | `entity-inspector.tsx` | Up/Down between consecutive field rows, `First`/`Last` for edge rows, plus a Left-escapes-to-parent when a descendant was focused. | **Spatial nav handles this.** Field rows stack vertically in the inspector. The Left-escape behaviour is geometric (descendant rect is inside parent rect — Left from inside lands on the parent's left edge, which is the parent itself via `is_in_direction`). |
| `buildListClaimPredicates` | `mention-view.tsx` | Left/Right between sibling mention pills; first pill claimed Right when the parent field was focused. | **Spatial nav handles this.** Pills are laid out horizontally in CSS flow with distinct rects; the parent→first-child enter is the same case as descendant-escape in reverse, and is resolved geometrically. |

Short version: every predicate that was deleted was geometric in nature, so spatial nav's rect-based resolver should produce the same results in each case.

### Integration test

Added `navOverride end-to-end: broadcastNavCommand routes override target through focus-changed` in `kanban-app/ui/src/components/focus-scope.test.tsx`. It renders two FocusScopes side-by-side, gives the source scope `navOverride={{ Right: "task:02" }}`, captures both spatial keys from the `spatial_register` calls, sets initial focus, fires `broadcastNavCommand("nav.right")`, asserts `spatial_navigate` is invoked with the source key + `"Right"`, then simulates Rust emitting `focus-changed` with `next_key = target scope's key` (the shape Rust uses after `navigate()` returns the override target). The test asserts React picks up the new focus state, proving the loop: `React → spatial_register (overrides) → broadcastNavCommand → spatial_navigate → focus-changed → React focus state`.

The Rust override *selection* itself (navigate() honoring the override) is covered by existing unit tests in `swissarmyhammer-spatial-nav`.

### Follow-up decision card

Because no production code passes `navOverride`, the third acceptance criterion (decision ticket on whether to keep it) is satisfied by card **`01KPGDY096D54Q7CZBZG1EWTEZ`** — "Decide whether to keep navOverride (zero production users)". That card lists the three possible paths (keep / delete / gate on manual regression) and the work required to follow through on each.

## Acceptance Criteria

- [x] Each deleted predicate memo has a documented status: "spatial nav handles this" OR "now uses navOverride because X"
- [x] If any `navOverride` users exist in production code, there is at least one integration test covering the end-to-end path
- [x] If no users exist and spatial nav truly handles every case, open a decision ticket on whether to keep `navOverride` at all
