---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd280
project: spatial-nav
title: 'Adopt unified spatial-nav policy: edge drill-out + zone-to-zone — supersedes per-direction tactical cards'
---
## What

The user has reported five spatial-nav bugs since the per-component cards were closed. They are all symptoms of the same architectural gap: **the kernel has no unified policy for what happens when navigation in a direction has no in-beam candidate at the focused entry's level.** The per-direction tactical cards each tried to fix their symptom in isolation; this card replaces all of them with one rule.

### The policy (per user, 2026-04-27)

**Edge drill-out + zone-to-zone traversal.**

When `BeamNavStrategy::next` runs from a focused entry and finds no in-direction candidate at the focused entry's level, it walks up the parent chain until either a candidate exists at some ancestor's level or the layer root is reached. In one rule:

1. Score the focused entry's same-level peers (siblings sharing `parent_zone`) for the requested direction. If a candidate satisfies the beam test, return it.
2. Otherwise, **escalate**: treat the focused entry's parent zone as the new search origin. Score the parent's same-level peers. Repeat.
3. If escalation reaches the layer root (or would cross a layer boundary) with no candidate, return `None`.

This collapses what's separate today:
- Leaf rule 1 (in-zone leaves) → the first level of the cascade.
- Leaf rule 2 (cross-zone leaf fallback in same layer) → goes away. Cross-column nav happens via escalation.
- Zone-only `navigate_zone` → also goes away. Zones use the same cascade.

### Test approach (decided 2026-04-27)

**Rust integration tests against realistic registry fixtures.** Source of truth is `cargo test -p swissarmyhammer-focus`. Browser tests demoted to wiring-only.

Reuse the **shared fixture builder** introduced by `01KQ7STZN3G5N2WB3FF4PM4DKX`:
`swissarmyhammer-focus/tests/fixtures/` — builds a `SpatialRegistry` matching what production React generates: window layer with `ui:navbar`, `ui:perspective-bar`, `ui:board` zones; `ui:board` contains `column:*` zones with `column.name` leaves and `task:*` cards; plus a separate `inspector` layer with `panel:*` zones containing `field:*` zones.

If `01KQ7STZN3G5N2WB3FF4PM4DKX` lands first, this card imports the fixture. If this card lands first, this card builds the fixture and the directional-nav card imports.

### The user trajectories the policy must satisfy

**Trajectory A — vertical traversal up the entity stack.**
- `nav("task:T1A", Up) == Some("column:TODO.name")`
- `nav("column:TODO.name", Up) == Some("column:TODO")` (escalate to column zone)
- `nav("column:TODO", Up) == Some("ui:perspective-bar")` (escalate to ui:board's level, find perspective bar above)
- `nav("ui:perspective-bar", Up) == Some("ui:navbar")`
- `nav("ui:navbar", Up) == None`

**Trajectory B — cross-column horizontal.**
- `nav("task:T1A", Right) == Some("column:DOING")` — kernel returns the next-column zone moniker; the React adapter handles drill-back-in if desired.
- Mirror left.

**Trajectory C — left from leftmost.**
- `nav("task:T1A", Left) == None` — escalation reaches `ui:board`, no left sibling at the layer root.

**Trajectory D — inspector field nav.**
- `nav("field:task:T1.title", Down) == Some("field:task:T1.status")`
- `nav("field:task:T1.status", Down) == Some("field:task:T1.assignees")`
- `nav("field:task:T1.assignees", Down) == None` — escalates to panel, no zone below within the inspector layer; **layer boundary guard** prevents crossing into the window layer.

### Files in scope

**Kernel:**
- `swissarmyhammer-focus/src/navigate.rs` — rewrite `BeamNavStrategy::next` as the cascade. Pseudo-code:

  ```rust
  fn next(&self, registry, focused, direction) -> Option<Moniker> {
      let mut current = registry.entry(focused)?;
      loop {
          if let Some(target) = beam_among_siblings(registry, current, direction) {
              return Some(target);
          }
          let Some(parent_key) = current.parent_zone() else { return None; };
          let Some(parent) = registry.entry(&parent_key) else { return None; };
          if parent.layer_key() != current.layer_key() { return None; }
          current = parent;
      }
  }
  ```

- Delete `navigate_leaf`, `navigate_zone`, the leaf-rule-2 fallback, and the zone-only beam helpers. One cascade replaces them all.

- Edge commands (`First`, `Last`, `RowStart`, `RowEnd`) keep their level-bounded behavior — no escalation cascade for those.

- Override (rule 0) still runs first.

**Tests (Rust, source of truth):**
- `swissarmyhammer-focus/tests/unified_trajectories.rs` (new) — runs trajectories A, B, C, D against the realistic fixture.

**React side:** no behavior change. The wiring works the same way — register zones/leaves, dispatch `spatial_navigate`. The kernel's answers change.

### What this card does NOT do

- Does not change keymap dispatch.
- Does not introduce auto-drill-in (kernel returns the zone moniker; subsequent press inside the zone walks deeper).
- Does not change `<FocusZone>` / `<FocusScope>` registration shape.
- Does not change override rule 0.
- Does NOT cross layer boundaries via escalation. The inspector is captured-focus.

## Acceptance Criteria

- [x] `swissarmyhammer-focus/src/navigate.rs` implements the cascade. `navigate_leaf` and `navigate_zone` are merged into one cascade. Leaf-rule-2 cross-zone leaf fallback is deleted.
- [x] Cascade respects layer boundaries: escalation never crosses from one `LayerKey` to another.
- [x] `swissarmyhammer-focus/tests/unified_trajectories.rs` runs all four trajectories against the realistic fixture builder. All asserts pass.
- [x] Edge commands (`First`, `Last`, `RowStart`, `RowEnd`) still operate on the focused entry's level only.
- [x] Override rule 0 still runs first.
- [x] Existing kernel tests pass; mechanism-specific tests (asserting "rule 2 fired") are updated to assert observable outcome instead.
- [x] `cargo test -p swissarmyhammer-focus -p swissarmyhammer-kanban` is green.
- [x] `cd kanban-app/ui && npm test` is green.

## Tests

### Rust integration tests (mandatory — source of truth)

`swissarmyhammer-focus/tests/unified_trajectories.rs`. Uses the shared realistic fixture builder.

Test cases:

1. `unified_trajectory_a_up_walks_card_to_header_to_column_to_perspective_bar_to_navbar`
2. `unified_trajectory_b_right_from_card_in_column_a_returns_column_doing_zone`
3. `unified_trajectory_c_left_from_leftmost_card_returns_none_at_layer_root`
4. `unified_trajectory_d_down_between_inspector_field_zones_with_layer_boundary_guard`

For trajectory D, **layer boundary guard** explicit assertion: `nav("field:task:T1.assignees", Down) == None` and is NOT any moniker from the window layer.

### Browser tests (wiring guards, supplementary)

No new browser-mode test file. Existing tests stay green; their docstrings get updated to reference this card's policy. No assertions on kernel mechanism — only on observable wiring (DOM `data-focused`, `mockInvoke` shape).

### How to run

```
cargo test -p swissarmyhammer-focus
cd kanban-app/ui && npm test
```

## Workflow

- Use `/tdd`. Order:
  1. Build (or import from `01KQ7STZN3G5N2WB3FF4PM4DKX`) the realistic fixture builder.
  2. Write the four trajectory tests. Confirm they fail against the current `navigate_leaf` / `navigate_zone` split.
  3. Rewrite `BeamNavStrategy::next` as the unified cascade with layer-boundary guard. Delete the old helpers. Confirm A/B/C/D pass.
  4. Run `cargo test -p swissarmyhammer-focus` to confirm no other kernel tests regress. Update any tests that asserted on rule-2 mechanism specifically.
  5. Update browser-mode tests' docstrings to reference this policy; remove any mechanism assertions there.
  6. Mark the superseded tactical cards as done.

## Review Findings (2026-04-27 16:30)

Cascade implementation verified correct against all four user trajectories. Geometry traced step-by-step against the realistic-app fixture; iter-0 and iter-1 candidate sets, beam scoring, drill-out fallback, and layer-boundary guard all match the spec. Edge commands stay level-bounded. Override rule 0 runs first. Same-kind filtering is sound (rationale documented; no legitimate candidate is eliminated against the production registry shape). Deleted helpers (`navigate_leaf`, `navigate_zone`, `beam_in_zone`, `beam_all_leaves_in_layer`, `beam_sibling_zones`, `edge_command_for_leaf`, `edge_command_for_zone`) have zero remaining `fn` definitions and zero callers in Rust source. JS shadow navigator faithfully mirrors the Rust kernel including same-kind filtering and drill-out fallback. `cargo check`, `cargo clippy --tests`, and the targeted `cargo test` for `unified_trajectories`, `navigate`, and `card_directional_nav` are all clean (41 tests pass).

### Nits

- [x] `kanban-app/ui/src/components/entity-card.tsx:65-73` — Comment block cites the deleted "rule 2 (cross-zone leaf fallback)" mechanism and `navigate_zone` helper as the reason cards are leaves, and references the now-deleted kernel test `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b`. The structural conclusion (cards-as-leaves) is still correct under the unified cascade, but the rationale and the dangling test reference are stale. Suggested fix: replace with a reference to the unified cascade's same-kind filtering rule (cards-as-leaves so iter-0 leaf candidates and iter-1 zone candidates work as the user expects), and point to `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone` in `swissarmyhammer-focus/tests/navigate.rs`. **Resolved 2026-04-27**: comment rewritten to describe the unified cascade's iter-0 / iter-1 trajectory and points at the renamed kernel test.
- [x] `kanban-app/ui/src/components/column-view.tsx:42-44` — Same stale "rule 2 (cross-zone leaf fallback)" / `navigate_zone` rationale as above. Suggested fix: rewrite to describe the unified cascade's iter-1 escalation (cards as leaves so iter 0 finds in-zone peers; iter 1 escalates to the column zone and lands on the next column). **Resolved 2026-04-27**: docstring rewritten to describe iter-0 in-column peers, iter-1 escalation to the parent column zone, and landing on the neighbouring column zone.
- [x] `kanban-app/ui/src/components/entity-card.spatial.test.tsx:7-13` — Same stale mechanism reference and same dangling test name (`rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b`). Suggested fix: update to reference the unified cascade's observable outcome and point at the renamed kernel test. **Resolved 2026-04-27**: top docstring rewritten to describe the iter-0 / iter-1 trajectory and points at `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`.
- [x] `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx:738` — Test name `"ArrowRight from task:T1 lands on a card in column DOING (cross-zone)"` says "lands on a card", but under the unified cascade the focused element will carry `data-moniker="column:DOING"` (a zone), not a card moniker. The assertion correctly accepts both shapes via `columnOfTaskMoniker`, but the title is misleading. Suggested fix: rename to `"ArrowRight from task:T1 lands on column DOING (zone or card)"` or similar. **Resolved 2026-04-27**: renamed to `"ArrowRight from task:T1 lands on column DOING (zone or card, unified cascade)"`.

## Review Findings (2026-04-28 11:35)

Verified: all four prior `Nits` are genuinely resolved at the cited line numbers. The renamed kernel test (`cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`) exists at `swissarmyhammer-focus/tests/navigate.rs:459`, so the redirected references in the rewritten docstrings actually resolve.

The implementer's scope-discipline call is **mostly defensible** — the truly other-file drift (`sortable-task-card.test.tsx`, `entity-card.test.tsx`, `inspector-focus-bridge.tsx`, `entity-inspector.tsx`, `column-view.spatial-nav.test.tsx`) is reasonably deferred to a follow-up cleanup card. Those touch files outside the immediate iter-0/iter-1 trajectory the prior nits called out. The kernel-side mentions in `swissarmyhammer-focus/src/navigate.rs:21`, `tests/navigate.rs:11`, and `tests/unified_trajectories.rs:23` are intentional historical context (describing what the cascade replaced) and should stay.

However, two stale blocks remain in the **same files** the implementer just touched, and they carry the **identical** stale rationale + dangling test name pattern that the prior review flagged. Fixing only the line range cited and ignoring the same defect twenty-eighty lines lower in the same file is line-number cargo-culting, not scope discipline. These need to be cleaned up before the task lands.

### Warnings

- [x] `kanban-app/ui/src/components/column-view.tsx:320-330` — The `ScopeRegisterEntry` interface docstring carries the **identical** stale block the implementer just rewrote at lines 42-52: `"Cards must be leaves so cross-column right/left navigation falls through to rule 2 (cross-zone leaf fallback) — see the docstring on <EntityCard> and the kernel test rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b."` Both the "rule 2 (cross-zone leaf fallback)" mechanism and the test `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b` no longer exist. Suggested fix: rewrite the closing sentences to mirror what the rewritten `<ColumnView>` props docstring now says — "Cards must be leaves so the unified cascade's iter-0 / iter-1 trajectory works as the user expects (iter 0 finds in-column card peers; iter 1 escalates to the column zone and lands on the neighbouring column zone). See the docstring on `<EntityCard>` and the kernel test `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`." **Resolved 2026-04-28**: `ScopeRegisterEntry` docstring rewritten using the unified-cascade vocabulary; rationale describes iter-0 in-column card peers and iter-1 escalation to the parent column zone, and the kernel-test pointer is redirected to `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`. Workspace grep confirms zero remaining `rule 2` / `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b` references in `column-view.tsx`.
- [x] `kanban-app/ui/src/components/entity-card.spatial.test.tsx:536-542` — The "test #1b" comment block (`does not register the card root as a FocusZone (the card is a leaf, not a zone)`) carries the **identical** stale block the implementer just rewrote at the top of the same file (lines 7-13): `"Cards must register as leaves so cross-column right/left navigation falls through to rule 2 (cross-zone leaf fallback). If the card ever flips back to being a zone, sibling-zones-only navigation would trap focus in the column. See the docstring on <EntityCard> and the kernel test rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b."` Same stale "rule 2" mechanism, same dangling test name. Suggested fix: rewrite to mirror the file's top docstring — describe the unified cascade's observable outcome (iter 0 in-column peers; iter 1 escalates to the column zone) and redirect the test reference to `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`. **Resolved 2026-04-28**: test #1b comment block rewritten to mirror the file's top docstring; describes iter-0 in-column peers and iter-1 escalation to the parent column zone, and redirects the kernel-test pointer to `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`. Workspace grep confirms zero remaining stale references in `entity-card.spatial.test.tsx`.

### Nits

- [x] Once the two warnings above are cleared, capture a single follow-up card for the residual cross-file doc drift the implementer correctly deferred: `kanban-app/ui/src/components/sortable-task-card.test.tsx:150-154`, `kanban-app/ui/src/components/entity-card.test.tsx:623,667-671`, `kanban-app/ui/src/components/inspector-focus-bridge.tsx:20`, `kanban-app/ui/src/components/entity-inspector.tsx:59`, `kanban-app/ui/src/components/column-view.spatial-nav.test.tsx:23`. All carry the same stale "rule 1 (in-zone)" / "rule 2 (cross-zone leaf fallback)" vocabulary and (in two cases) the dangling test name. Out of scope for this card; tracked for cleanup so the search-and-replace doesn't get lost. **Captured 2026-04-28**: deferred sweep tracked on follow-up card `01KQ9X7F7YNG4NE4AGBNZJMSWG` ("Sweep residual stale \"rule 1 / rule 2\" / dangling test-name doc drift across remaining spatial-nav consumers"). All five files plus the replacement-text recipe are listed there so the sweep can land cleanly without re-deriving the rewrite voice.

## Implementer Notes (2026-04-28)

Same-file sweep applied per the reviewer's scope-discipline call:

- `kanban-app/ui/src/components/column-view.tsx` — `ScopeRegisterEntry` docstring rewritten (mirrors the `<ColumnView>` props docstring's iter-0 / iter-1 vocabulary).
- `kanban-app/ui/src/components/entity-card.spatial.test.tsx` — test #1b comment block rewritten (mirrors the file's top docstring).
- Workspace grep confirms zero remaining `rule 2` / `rule_2_realistic_board_right_from_card_in_a_lands_on_card_in_b` matches in either file.
- Kernel-side mentions in `swissarmyhammer-focus/src/navigate.rs:21`, `tests/navigate.rs:11`, and `tests/unified_trajectories.rs:23` are intentional historical context describing what the cascade replaced — left untouched per the reviewer's note.

### Implementer Verification (2026-04-28)

- `pnpm tsc --noEmit` — exit 0, no diagnostics.
- `pnpm vitest run` — 160 test files; **1754 passed | 1 skipped** (matches the gate the reviewer set).
- `cargo test -p swissarmyhammer-focus` — 12 test binaries, **134 tests passed | 0 failed**, including the 4 unified-trajectory tests and the 8 override tests.
- `cargo build --workspace` — clean (`Finished dev profile`).