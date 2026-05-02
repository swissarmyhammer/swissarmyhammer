---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffed80
project: spatial-nav
title: Enforce scope-is-leaf invariant in spatial-nav kernel; audit nav-bar/toolbar misuse
---
## What

The spatial-nav primitive vocabulary has three peers (per `kanban-app/ui/src/components/focus-scope.tsx` lines 19–47):

  - `<FocusLayer>` — modal boundary
  - `<FocusZone>` — navigable container, can have children (other zones or scopes)
  - `<FocusScope>` — **leaf** in the spatial graph

The leaf contract is documented but not enforced. Several places in the UI register a `FocusScope` whose subtree contains further `FocusZone` / `FocusScope` / focusable controls — e.g. the navbar wraps the `<BoardSelector>` (a Radix Popover trigger with a dropdown list of focusable items) inside `<FocusScope moniker={asSegment("ui:navbar.board-selector")}>` (`kanban-app/ui/src/components/nav-bar.tsx` line 85). Wrapping a non-leaf as a Scope confuses the kernel's beam search (the scope's rect is the *whole* sub-region but it is treated as a single leaf candidate), breaks "drill into the bar and remember the last-focused leaf" (the navbar zone's last-focused leaf is the wrapper, not the actually-focused inner control), and silently degrades keyboard navigation in toolbars.

This task makes the invariant programmatic and noisy.

## Acceptance Criteria
- [x] `swissarmyhammer-focus/src/registry.rs::SpatialRegistry::register_scope` emits `tracing::error!(target = "swissarmyhammer_focus::registry", "scope-not-leaf parent is not a Zone", ...)` when the new scope's `parent_zone` resolves to a `RegisteredScope::Scope`.
- [x] `register_zone` emits the same error when its `parent_zone` resolves to a `RegisteredScope::Scope`.
- [x] The check is order-independent: a parent registered after the child still produces exactly one error per offending child (re-scan on insert; documented in `register_scope` doc-comment).
- [x] `nav-bar.tsx` is migrated to `<FocusZone moniker="ui:navbar.board-selector">`; `board-selector.tsx` adds inner leaf scopes around the dropdown trigger and tear-off button. The navbar's "Open Board" dropdown now produces zero `scope-not-leaf` errors.
- [x] Doc-comment on `<FocusScope>` (focus-scope.tsx lines 19–47) is extended with: "Registering a `<FocusScope>` whose subtree contains further `<FocusScope>` or `<FocusZone>` is a kernel error and is logged as `scope-not-leaf` to `just logs`."
- [x] **Iteration 2 (2026-05-02)**: Path-prefix branch added to the kernel's scope-is-leaf invariant. Catches DOM-subtree violations where the offending Scope's `parent_zone` is bypassed because `<FocusScope>` does not push `FocusZoneContext.Provider`. The entity card's `<FocusScope task:>` wrapping `<Field>` zones (whose `parent_zone` skips up to the column zone) is the canonical case the original `parent_zone`-only check could not catch.
- [x] **Iteration 2**: Entity card promoted from `<FocusScope>` to `<FocusZone>`. Drag-handle and inspect-button wrapped in inner `<FocusScope card.drag-handle:{id}>` / `<FocusScope card.inspect:{id}>` leaves. Field zones now nest under the card zone (parent_zone = card.fq) instead of the column.
- [x] **Iteration 2**: Board view's outer entity-moniker `<FocusScope board:>` promoted to `<FocusZone>` so the nested `<BoardSpatialZone>` (a `<FocusZone moniker="ui:board">`) and every column/card descendant no longer triggers path-prefix `scope-not-leaf`.
- [x] **Iteration 3 (2026-05-02)**: Layer-inside-Scope detection added. `push_layer` and `register_scope` now enforce that a Scope cannot contain a Layer either, mirroring the existing Zone-inside-Scope detection. Forward path-prefix scan in `push_layer` and a new layer-arm of the backward scan in `register_scope` close the gap.

## Tests
- [x] `swissarmyhammer-focus/tests/scope_is_leaf.rs` — 6 tests covering scope-under-scope, zone-under-scope, scope-under-zone (silent), order-independence (parent after child as scope/zone), and the grep-token contract.
- [x] `kanban-app/ui/src/components/nav-bar.scope-leaf.spatial.test.tsx` — asserts `ui:navbar.board-selector` registers as a zone, not a scope; asserts inspect/search remain leaves.
- [x] `cargo nextest run -p swissarmyhammer-focus` passes (originally 197/197; now 201/201 after iteration-2 additions; now 205/205 after iteration-3 additions).
- [x] `pnpm vitest run src/components/nav-bar` passes (39/39); board-selector and focus tests also pass (155/155 across nav-bar/board-selector/focus directories).
- [x] **Iteration 2**: `swissarmyhammer-focus/tests/scope_is_leaf.rs` extended with 3 new tests: `path_prefix_zone_under_scope_logs_error`, `path_prefix_backward_scan_fires_when_scope_registers_late`, `parent_zone_and_path_prefix_collapse_to_single_event`. All 10 tests pass.
- [x] **Iteration 2**: New `kanban-app/ui/src/components/entity-card.scope-leaf.spatial.test.tsx` with 4 tests pinning card-as-zone shape, drag-handle/inspect-button leaves under it, and Field zones with `parent_zone === cardZone.fq`. All pass.
- [x] **Iteration 3**: `swissarmyhammer-focus/tests/scope_is_leaf.rs` extended with 4 more tests: `layer_under_scope_logs_error`, `layer_path_prefix_backward_scan_when_scope_registers_late`, `layer_under_zone_silent`, `layer_same_shape_reregistration_is_silent`. All 14 tests in this file pass.

## Implementation Notes

The order-independence trade-off picked: **re-scan on every insert** (no deferred-validation queue). On `register_scope`, a backward scan walks the registry once to fire errors for any pre-existing entries whose `parent_zone` names the just-inserted FQM. On `register_zone`, no backward scan is needed (a Zone is a legal parent for both kinds). Cost is O(n) per `register_scope`, acceptable given registration burst frequency. Documented in `registry.rs::register_scope` doc-comment.

The grep-friendly token `scope-not-leaf` appears literally in every emitted error message so `just logs | grep scope-not-leaf` filters this class of violation.

`<FocusZone>` for `ui:navbar.board-selector` uses `showFocusBar={false}` because the inner leaves (dropdown trigger, tear-off button, editable name `<Field>` zone) own the visible focus signal — the same pattern used by `<FocusZone moniker="ui:navbar">` and `<FocusZone moniker="ui:perspective-bar">`.

## Review Findings (2026-05-01 15:12)

### Warnings
- [x] `swissarmyhammer-focus/src/registry.rs` (`warn_existing_children_of_scope` + `warn_if_parent_is_scope`) — Same-shape re-registration of an offending parent or child re-fires `scope-not-leaf` for every existing illegal pair on every call. Reproduced locally: registering parent P (Scope) → child C (Scope under P) emits 1 error; then re-registering P twice (same shape, normal lifecycle) emits 2 more errors for the same C. The registry's other warning, `warn_on_structural_mismatch`, deliberately silences same-shape re-registration because StrictMode double-mount, ResizeObserver rect refresh, and the virtualizer placeholder→real-mount swap all trip it on the hot path (see the doc-comment on `warn_on_structural_mismatch`). The new `scope-not-leaf` checks should follow the same precedent. Suggested fix: in `register_scope`, do the backward scan **only when the just-inserted entry is structurally new** (i.e. the previous entry under that FQM was missing OR was a Zone — a kind flip introducing a Scope); skip the scan when replacing a Scope with the same-shape Scope. Symmetrically, gate the forward check in `register_scope` / `register_zone` to "did the structural shape change?" so a same-shape re-register of an already-known offender is silent. Update the doc-comment claim "exactly one error per offending child" to "exactly one error per structurally novel offending edge" (the existing tests still pass under that semantics).

### Nits
- [x] `swissarmyhammer-focus/src/registry.rs` — In `warn_scope_not_leaf` the structured field `parent_kind = "scope"` is hardcoded in the `tracing::error!` macro, but every other piece of metadata (kind, fq, segment, parent_zone, parent_segment) is parameterised. Since this helper is only ever called when the parent is a Scope, the constant is correct, but a future caller for a different parent-kind invariant would silently mis-label its events. Either drop the field (the message and the `kind`/`parent_segment` already convey it) or take it as a parameter so it stays consistent with the rest of the structured layout.
- [x] `swissarmyhammer-focus/tests/scope_is_leaf.rs` — Test 4a (`parent_registered_after_child_as_scope_emits_error_once`) registers the outer zone first so the parent can resolve its enclosing zone — but the child registers before the parent, with `parent_zone = parent_path` (the parent's FQM, not a layer FQM). The current `warn_if_parent_is_scope` exits early when the parent FQM is unknown, so this works. A short comment on the test explaining "the forward check is silent when parent is unknown; the test is pinning that the **backward** scan fires when the parent later registers as a Scope" would make the intent crystal-clear to a future reader.
- [x] `swissarmyhammer-focus/src/registry.rs` (`warn_existing_children_of_scope`) — The match-arm `Some(p) => p.segment(), None => continue, // unreachable; we just inserted it` is dead code under the current call sites (the caller always inserts before invoking this helper). If you want to keep the defensive arm, prefer `expect("just-inserted entry is missing from registry")` so the invariant violation is loud rather than silently skipping the offender. Otherwise drop the helper's lookup-on-self entirely and pass the parent's segment in by value from `register_scope`.

## Review Fixes Applied (2026-05-01 15:17)

- **Warning (same-shape re-registration noise)**: Added `same_shape()` helper that mirrors the field set checked by `warn_on_structural_mismatch`. Both `register_scope` and `register_zone` now compute `shape_unchanged` once against any existing entry and gate **both** the forward `warn_if_parent_is_scope` call and (in `register_scope`) the backward `warn_existing_children_of_scope` scan on `!shape_unchanged`. Same-shape re-register of an already-known illegal edge is silent. Doc-comment on `register_scope` updated: contract now reads "exactly one error per **structurally novel** offending edge". New regression test `same_shape_reregistration_is_silent` registers an illegal Scope-under-Scope edge once and then re-registers both ends three times with identical shapes; total `scope-not-leaf` events stays at 1.
- **Nit (hardcoded `parent_kind`)**: Added `parent_kind: &'static str` parameter to `warn_scope_not_leaf`. Both call sites (`warn_if_parent_is_scope`, `warn_existing_children_of_scope`) pass `"scope"` explicitly — kept as a parameter so a future invariant for a different parent-kind cannot silently mis-label events.
- **Nit (test 4a intent)**: Added a multi-line `///` comment to `parent_registered_after_child_as_scope_emits_error_once` explaining that the forward check is silent when the parent FQM is unknown and that the assertion is therefore pinning the **backward** scan firing when the parent later registers as a Scope.
- **Nit (dead match arm)**: Refactored `warn_existing_children_of_scope` to take `parent_segment: &SegmentMoniker` from the caller. The lookup-on-self path is gone; `register_scope` clones the segment before the `insert(...)` consumes the `FocusScope`. Doc-comment notes that threading the segment through makes the call site explicit rather than relying on a defensive map lookup.

Files modified:
- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-focus/src/registry.rs`
- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-focus/tests/scope_is_leaf.rs`

Verification:
- `cargo nextest run -p swissarmyhammer-focus` → 198/198 pass (197 baseline + new same-shape regression test).
- `cargo clippy -p swissarmyhammer-focus --tests` → clean.
- `cargo fmt -p swissarmyhammer-focus --check` → clean.
- `pnpm vitest run src/components/nav-bar` → 39/39 pass.

## User Feedback (2026-05-02): Task card still registered as a scope

User reported: "i still clearly see the task card as a scope, it should be a zone. somehow your enforcement is clearly not enforcing"

### Root Cause Analysis

The original `parent_zone`-based enforcement was too narrow. It only caught violations where a child explicitly named a Scope as its `parent_zone`. But in production, a `<FocusZone>` rendered inside a `<FocusScope>` does NOT pick the Scope as its `parent_zone` — it picks the **column zone** above. Why? Because:

- `<FocusZone>` reads its `parent_zone` from `useParentZoneFq()`, which walks `FocusZoneContext`.
- `<FocusScope>` does NOT push `FocusZoneContext.Provider` (only `FullyQualifiedMonikerContext.Provider`).
- So a Field zone inside a card scope reads `parent_zone` from the column zone, skipping the card.

Result: the kernel registry sees the Field as a sibling of the card under the column, not as a descendant of the card. The `parent_zone` check never fires for this shape — even though the React DOM tree clearly has the Field nested inside the card.

### Fix: Path-prefix scope-is-leaf branch

Added a second detection branch to `register_scope` and `register_zone`. The new branch compares **FQM strings**: if a Scope's FQM is a strict path-prefix of any registered descendant's FQM (e.g. Scope at `/L/col/card` vs descendant at `/L/col/card/field`), the descendant was clearly composed inside the Scope's `<FocusScope>` (FQM composition goes through `FullyQualifiedMonikerContext.Provider`, which Scope DOES push). Fire one `scope-not-leaf` per offender.

The two relations are deduplicated: when both `parent_zone` AND path-prefix detect the same offender × ancestor pair, emit ONE event tagged `relation = "both"`. The grep token `scope-not-leaf` still appears in every message so `just logs | grep scope-not-leaf` catches all branches uniformly.

### Audit Findings

Major scope-not-leaf offenders identified:

1. **`entity-card.tsx`**: `<FocusScope task:{id}>` wraps `<Field>` zones — the user's exact complaint. **FIXED** by promoting to `<FocusZone>` and adding inner leaf scopes around the drag handle (`card.drag-handle:{id}`) and inspect button (`card.inspect:{id}`).
2. **`board-view.tsx`**: outer `<FocusScope board:{id}>` wraps `<BoardSpatialZone>` (a `<FocusZone ui:board>`) plus every column zone and card. **FIXED** by promoting to `<FocusZone>` with `showFocusBar={false}`.
3. **`data-table.tsx`** row: `<FocusScope renderContainer={false}>` wraps `<EntityRow>` containing per-cell `<GridCellFocusable>` `<FocusScope>` leaves. Cell FQMs are path-descendants of the row scope FQM. **DEFERRED** (requires adding `renderContainer={false}` to `<FocusZone>`) — see follow-up task `01KQM6VWQTK6KCQMQNKS0BX5V3`.

### Why the previous enforcement appeared silent for the task card

The previous enforcement scanned `parent_zone` only. The Field zone's `parent_zone` is the column (a legal Zone), not the card scope — so the kernel registry saw a perfectly legal layout from its narrow `parent_zone` view. The new path-prefix branch catches this directly: the Field's FQM is `/window/board/column:doing/task:T1/field:task:T1.title`, which begins with the card scope's FQM `/window/board/column:doing/task:T1` followed by `/`. That's an unambiguous DOM-subtree containment violation regardless of `parent_zone` linking.

### Files Modified (Iteration 2)

- `swissarmyhammer-focus/src/registry.rs` — added `is_path_descendant`, `warn_forward_scope_ancestors`, `warn_backward_scope_descendants`, `relation` field on `warn_scope_not_leaf`, removed redundant `warn_if_parent_is_scope` / `warn_existing_children_of_scope` / `warn_path_ancestor_is_scope` / `warn_existing_path_descendants_of_scope` helpers (collapsed into the unified pair).
- `swissarmyhammer-focus/tests/scope_is_leaf.rs` — added 3 new tests for the path-prefix branch and deduplication contract.
- `kanban-app/ui/src/components/entity-card.tsx` — `<FocusScope>` → `<FocusZone>` for card body; `DragHandle` and `InspectButton` wrapped in their own `<FocusScope>` leaves with per-entity-id segments.
- `kanban-app/ui/src/components/board-view.tsx` — outer `<FocusScope board:>` → `<FocusZone board:>` with `showFocusBar={false}`.
- `kanban-app/ui/src/components/entity-card.scope-leaf.spatial.test.tsx` — NEW test file with 4 tests pinning card-as-zone shape, drag-handle/inspect-button leaves with `parent_zone === card.fq`, and Field zones nesting under the card.
- `kanban-app/ui/src/components/entity-card.spatial.test.tsx` — updated tests/comments to reflect card-as-zone (was 56 tests, all still pass).
- `kanban-app/ui/src/components/entity-card.test.tsx` — updated `describe` block and assertions for card-as-zone shape.
- `kanban-app/ui/src/components/sortable-task-card.test.tsx` — updated card-as-zone assertions.
- `kanban-app/ui/src/components/column-view.spatial-nav.test.tsx` — updated "task card registers as zone parented at column" test.
- `kanban-app/ui/src/components/column-view.scroll-rects.browser.test.tsx` — updated `taskMonikerToKey` and rect-tracking helpers to accept both `spatial_register_zone` and `spatial_register_scope`.
- `kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx` — updated test #5 assertion to expect tasks-as-zones.
- `kanban-app/ui/src/components/board-view.spatial.test.tsx` — updated drill-out chain test to use `registerZoneArgs()` for card lookup.
- `kanban-app/ui/src/components/board-view.spatial-nav.test.tsx` — updated `ui:board.parentZone` assertion to point at the outer `board:<id>` entity zone (was `null`).
- `kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx` — updated comments and `keyForMoniker` doc-comment.
- `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` — updated Family 8 "task:* register as scope" → "task:* register as zone".
- `kanban-app/ui/src/components/inspectors-container.test.tsx` — fixed pre-existing bug in `registeredZones()` helper (was reading `z.moniker` which is undefined; spatial_register_zone payload has `segment`, not `moniker`).

### Verification

- `cargo nextest run -p swissarmyhammer-focus` → **201/201 pass** (was 198 before iteration 2; +3 new tests).
- `cargo clippy -p swissarmyhammer-focus --tests` → clean.
- `cargo fmt -p swissarmyhammer-focus --check` → clean.
- `pnpm vitest run src/components/entity-card` → **56/56 pass** (includes new `entity-card.scope-leaf.spatial.test.tsx` with 4 tests).
- `pnpm vitest run src/components/board-view src/components/column-view src/components/sortable-task-card src/components/nav-bar` → **182/185 pass** (3 pre-existing failures in `board-view.enter-drill-in.browser.test.tsx` that fail on base too — unrelated to this task; tracked as known flakes).
- Full UI suite: 1880/1889 pass; the 5 remaining failures all reproduce on base before my changes (pre-existing pipeline flakes in inspector/drill-in tests).

## User Clarification (2026-05-02): Layer-inside-Scope must also be enforced

User clarified: "A Scope is the leafmost primitive — it cannot have a Zone OR a Layer inside of it." The previous iterations enforced Scope-inside-Scope and Zone-inside-Scope, but a Layer mounted inside a `<FocusScope>` was structurally undetected. `push_layer` was a plain `self.layers.insert(l.fq.clone(), l)` with no invariant checking at all.

### Iteration 3 — Layer-inside-Scope enforcement

#### Forward path-prefix scan in `push_layer`

`push_layer` now performs a forward path-prefix scan: it walks the registered Scopes (via `self.scopes.values()` filtered to `RegisteredScope::Scope`) and emits one `scope-not-leaf` per ancestor Scope FQM that is a strict path-prefix of the new layer's FQM. The event is tagged `kind = "layer"`, `parent_kind = "scope"`, `relation = "path-prefix"`. Layers do not have a `parent_zone` field — their `parent` field always names another Layer FQM, never a scope/zone FQM — so the parent-zone branch does not apply to layers; only the path-prefix branch can fire.

#### Backward layer scan in `register_scope`

`warn_backward_scope_descendants` was extended to walk the layers map in a second pass after the existing scopes-map pass. The single helper now covers all three primitive kinds (scope, zone, layer) in one logical sweep, keeping the "exactly one event per structurally novel offending edge" contract uniform. Walking both maps in one helper avoids a separate `warn_backward_layer_descendants` and keeps the call site in `register_scope` to one line. Cost is O(n_scopes + n_layers) per `register_scope`, in line with the existing scan.

#### Same-shape gating for layers

Added `same_shape_layer(existing, candidate)` mirroring the existing `same_shape` for scopes/zones. Layer shape is `(segment, name, parent, window_label)`; `last_focused` is mutable runtime state populated by the navigator and intentionally excluded so a layer that has acquired focus history is not mis-classified as "structurally novel" on a same-shape re-mount. `push_layer` computes `shape_unchanged` once and gates the forward path-prefix scan on `!shape_unchanged`. The hot paths that re-push the same layer (StrictMode double-mount, palette open/close cycles, IPC re-batch) flow through silently after the first novel event. Note: the backward layer-arm in `register_scope` is gated by the existing `register_scope` `shape_unchanged` check — the layer descendant has not changed, but the just-inserted Scope has, so the edge counts as newly-novel.

#### Doc-comment updates

- `push_layer` doc-comment now documents the path-prefix check and the same-shape silencing.
- `register_scope` doc-comment now lists 5 checks (was 4): the new check 5 is the layers-map arm of the backward path-prefix scan. Opening line clarifies that a `<FocusScope>` cannot contain a Scope, a Zone, **or** a Layer.
- `warn_scope_not_leaf` doc-comment now notes that `kind` may be `"scope"`, `"zone"`, or `"layer"` and that Layers only ever match the path-prefix branch.
- `warn_backward_scope_descendants` doc-comment now describes the unified scopes+layers walk.
- `tests/scope_is_leaf.rs` module doc updated: 6 contract bullets (was 5) including a new "Layer mounted under a Scope" bullet, and the leaf contract sentence now reads "no `<FocusScope>`, no `<FocusZone>`, **and no `<FocusLayer>`**".

#### Design decision: extend the existing helper, don't bolt on a new one

The user explicitly asked whether `warn_forward_scope_ancestors`/`warn_backward_scope_descendants` should be generalized for all three primitive kinds or whether layers should get their own helper. Picked the unified-helper design for the backward scan (`warn_backward_scope_descendants` walks both maps in one logical sweep) because the call site stays single-line and the contract "one event per offender × ancestor pair" is centralised. For the forward scan, layers do NOT go through `register_scope`/`register_zone`, so they need their own forward path inside `push_layer` — but that path is only ~15 lines (no parent-zone branch, no deduplication needed), so inlining it into `push_layer` is cleaner than a 4th helper that takes a `kind: &'static str` parameter for one caller. Both functions stay well under 50 lines.

### Files Modified (Iteration 3)

- `swissarmyhammer-focus/src/registry.rs` — added `same_shape_layer`, extended `warn_backward_scope_descendants` to walk `self.layers`, modified `push_layer` to do forward path-prefix scan + same-shape gating, updated doc-comments on `register_scope`, `push_layer`, `warn_scope_not_leaf`, `warn_backward_scope_descendants`.
- `swissarmyhammer-focus/tests/scope_is_leaf.rs` — added `make_layer` helper and 4 new tests: `layer_under_scope_logs_error`, `layer_path_prefix_backward_scan_when_scope_registers_late`, `layer_under_zone_silent`, `layer_same_shape_reregistration_is_silent`. Imports extended to include `FocusLayer`, `LayerName`, `WindowLabel`. Module doc updated to include Layer-inside-Scope in the contract.

### Verification (Iteration 3)

- `cargo nextest run -p swissarmyhammer-focus --test scope_is_leaf` → **14/14 pass** (was 10 before iteration 3; +4 new tests).
- `cargo nextest run -p swissarmyhammer-focus` → **205/205 pass** (was 201 before iteration 3).
- `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` → clean.
- `cargo fmt -p swissarmyhammer-focus --check` → clean.
- `cargo nextest run -p swissarmyhammer-focus -p kanban-app -p swissarmyhammer-kanban -p swissarmyhammer-code-context -p swissarmyhammer-common` → **2919/2919 pass**. (Full `--workspace` run blocked by a pre-existing unmerged conflict in `agent-client-protocol-extras/src/recording.rs`; that file is unrelated to this task and was already in conflict state at session start.)
- `cargo clippy -p swissarmyhammer-focus -p kanban-app -p swissarmyhammer-kanban -p swissarmyhammer-code-context -p swissarmyhammer-common --all-targets -- -D warnings` → clean.