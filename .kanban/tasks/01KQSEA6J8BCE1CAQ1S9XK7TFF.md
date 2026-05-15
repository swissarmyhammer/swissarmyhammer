---
assignees:
- claude-code
depends_on:
- 01KQSDP4ZJY5ERAJ68TFPVFRRE
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8880
project: spatial-nav
title: 'Spatial-nav follow-up A: collapse FocusZone in kernel (scope/registry/navigate/lib)'
---
## Reference

Parent: `01KQSDP4ZJY5ERAJ68TFPVFRRE` — single-PR collapse was measured at 143 source files affected and split into 4 sequential sub-tasks (A/B/C/D). This is sub-task **A**.

After this task lands, the kernel crate compiles standalone with `FocusScope` as the only primitive. Other crates (`kanban-app`, `kanban-app/ui`) intentionally break — sub-task B fixes the IPC bridge, C sweeps React components, D sweeps tests + docs.

## What

Pure refactor inside `swissarmyhammer-focus` only. No behaviour change; the geometric algorithm and drill / first / last operations already don't distinguish kind — this just removes the type-level distinction.

### Files modified

- `swissarmyhammer-focus/src/scope.rs`
  - Deleted `FocusZone` struct and the entire `RegisteredScope` internal enum.
  - `FocusScope` is the single primitive; `last_focused: Option<FullyQualifiedMoniker>` moved onto it (defaults to `None`; populated when a child scope acquires focus).
  - `is_zone()`, `as_zone()`, `as_scope()`, `is_scope()` accessors all gone.
  - `ScopeKind`, `ChildScope`, `FocusEntry` enums all gone.

- `swissarmyhammer-focus/src/registry.rs`
  - Single internal `scopes: HashMap<FullyQualifiedMoniker, FocusScope>` collection (no separate `zones` / `leaves` split).
  - `leaves_iter` / `zones_iter` / `leaves_in_layer` / `zones_in_layer` collapsed to `scopes_iter` / `scopes_in_layer`.
  - `children_of_zone` renamed to `children_of`; new `has_children(parent_fq) -> bool` shorthand.
  - `register_scope` is the only registration path (no more `register_zone`).
  - `RegisterEntry` is now a single struct (no `kind` discriminator on the wire).
  - `BatchRegisterError` and `ScopeKind` are gone — `apply_batch` returns `()`.
  - Deleted ~150 LOC of scope-not-leaf validation: `warn_scope_not_leaf`, `warn_existing_children_of_scope`, `is_path_descendant`, `same_shape`, the forward/backward path-prefix scans, the parent-zone scan.
  - `first_child_by_top_left` and `last_child_by_bottom_right` retained; signatures updated to `&FocusScope`.

- `swissarmyhammer-focus/src/navigate.rs`
  - `geometric_pick`'s leaf-tie-break now uses "no registered children" via `reg.has_children(&cand.fq)` instead of `is_zone()`. The `BestCandidate` struct's `is_zone` field is now `has_children` with the same tie-break semantics (a leaf wins over a container).
  - All internal types use `&FocusScope` instead of `&RegisteredScope`.
  - `drill_in` / `drill_out` / `edge_command` carried over unchanged in shape — they walk children-by-parent_zone, no kind enum dependency.

- `swissarmyhammer-focus/src/state.rs`
  - Internal `ScopeVariant` enum and `prefer_variant` knob on `nearest_in_zone` deleted. Rule 1 / rule 2 cascade simplified accordingly.
  - `resolve_fallback` keys on `find_by_fq` instead of `entry()`.

- `swissarmyhammer-focus/src/lib.rs`
  - `pub use` updated: `FocusScope`, `FocusLayer`, `RegisterEntry`, `SpatialRegistry`. Removed `FocusZone`, `ScopeKind`, `ChildScope`, `FocusEntry`, `BatchRegisterError`.
  - Crate-root prose rewritten for the single-primitive model.

- `swissarmyhammer-focus/src/types.rs`
  - Stale `<FocusZone>` doc reference dropped.

### Tests

- Deleted `tests/scope_is_leaf.rs` — its premise (a `FocusScope` cannot wrap a `FocusZone` or other `FocusScope`) is gone under the unified primitive.
- Rewrote `tests/batch_register.rs` — the `RegisterEntry` enum is now a struct; the `BatchRegisterError::KindMismatch` test cases are vacuous and were removed. Wire-shape and idempotency assertions retained.
- Updated `tests/fixtures/mod.rs` — `make_zone` and `make_leaf` produce identical `FocusScope` values; the kernel decides leaf vs container at runtime by what registers under it.
- Mass updates to `tests/*.rs` (16 files) — `register_zone` → `register_scope`, `FocusZone { … }` → `FocusScope { …, last_focused: None }`, accessor method calls (`.fq()`, `.segment()`, `.rect()`, `.layer_fq()`, `.parent_zone()`, `.overrides()`) → field access, registry method renames (`reg.zone()` / `reg.entry()` → `reg.find_by_fq()`, `entries_in_layer` → `scopes_in_layer`, `children_of_zone` → `children_of`).
- Reframed two tests in `tests/overlap_tracing.rs` (`zone_and_scope_same_xy_does_not_warn` and `scope_and_zone_same_xy_does_not_warn`) as a single `same_xy_overlap_warns_regardless_of_descendants` because cross-kind-suppression is no longer a meaningful semantic — every same-(x, y) overlap now warns uniformly.
- Reframed `fixture_navbar_has_three_leaves_and_one_field_zone` to `fixture_navbar_has_four_children` and `fixture_perspective_bar_has_three_tab_leaves` to `fixture_perspective_bar_has_three_tab_children` — the kernel doesn't track "is leaf" at registration; `has_children` is the runtime check.
- Reframed `registry_returns_typed_accessor_for_each_variant` → `registry_returns_scope_for_each_registration`, exercising `has_children` instead of variant-typed accessors.
- Reframed `leaves_and_zones_in_layer_filter_by_layer_fq` → `scopes_in_layer_filter_by_layer_fq` to use the unified iterator.
- Removed `structural_mismatch_kind_flip_emits_error` — kind flipping is no longer a representable bug.

## Acceptance Criteria

- [x] `FocusZone` struct is deleted from `swissarmyhammer-focus/src/scope.rs`.
- [x] `is_zone()` accessor is gone; replaced by `children_of(fq).next().is_some()` / `has_children(fq)` at every callsite.
- [x] `ScopeKind::Zone`, `ChildScope::Zone`, `FocusEntry::Zone`, `BatchRegisterError::KindMismatch` enum variants are gone.
- [x] `register_zone` IPC handler is gone — `register_scope` is the only registration path.
- [x] The `scope-not-leaf` validation path (`warn_scope_not_leaf`, `warn_existing_children_of_scope`, `is_path_descendant`, `same_shape`) is deleted.
- [x] `geometric_pick`'s leaf-tie-break uses "no registered children" instead of `is_zone()`.
- [x] `cargo test -p swissarmyhammer-focus` passes — all kernel-internal tests updated and green.
- [x] `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` is clean.
- [x] `grep -r "FocusZone\|is_zone\|register_zone\|ScopeKind::Zone\|BatchRegisterError::KindMismatch" swissarmyhammer-focus/src` returns no source-code matches.
- [x] `cargo build --workspace` is INTENTIONALLY broken in `kanban-app` and `kanban-app/ui` build steps — sub-task B picks them up. The kernel itself compiles cleanly.

## Tests

- [x] All existing `swissarmyhammer-focus/tests/*.rs` integration tests pass after the rename sweep — same behaviour, same assertions where they don't reference the deleted kind distinction.
- [x] The four cross-zone regressions in `tests/cross_zone_geometric_nav.rs` still pass.
- [x] The drill / first / last assertions in `tests/drill.rs` still pass.
- [x] `tests/in_zone_any_kind_first.rs` — kept as-is; its assertions about cardinal navigation crossing kinds still hold under the unified primitive (the test's name is now slightly anachronistic but the geometric assertions are unchanged).
- [x] `cargo test -p swissarmyhammer-focus`: zero failures, zero warnings.

## Workflow

- This sub-task is a mechanical rename + delete inside one crate. The existing test suite is the safety net; no `/tdd` needed.
- Build incrementally: collapse the structs first, fix `cargo check -p swissarmyhammer-focus`, then update the kernel-internal tests, then run the full focus test suite.
- Do NOT touch `kanban-app/` or `kanban-app/ui/` — sub-tasks B/C/D handle those.
- If you find a callsite that does something genuinely zone-specific that's hard to translate, STOP and report — do not improvise.
#spatial-nav-redesign

## Review Findings (2026-05-03 14:50)

### Warnings
- [x] `swissarmyhammer-focus/src/scope.rs:65` and `swissarmyhammer-focus/src/registry.rs:142` — Doc-comments claim `last_focused` is "populated by the navigator on focus changes inside the scope/layer", but no kernel code ever writes to `FocusScope.last_focused` or `FocusLayer.last_focused`. The only writer in `register_scope` (registry.rs:508) preserves an *existing* value across a re-registration; it never originates one. This means the `FallbackParentZoneLastFocused` and `FallbackParentLayerLastFocused` cascade arms in `state.rs::resolve_fallback` are unreachable in production (only test fixtures that hand-populate the slot can exercise them). This is **pre-existing** — the old `FocusZone.last_focused` had the same situation, so the collapse preserves behaviour exactly and meets the "no behaviour change" criterion of sub-task A. But the contract claim in the docstrings is now stale-by-aspiration: either add a `record_focus(fq)` write hook into `SpatialState::focus`/`focus_layer` and have it bubble up to update each ancestor scope's slot, OR soften the docstring to "reserved; populated externally by the focus tracker once the wire-up exists." Track in a follow-up since the unified primitive made this drift more visible (one slot on every scope, vs. zones-only previously).
  - **Resolution**: Softened the docstrings on `FocusScope.last_focused` (`scope.rs`), `FocusLayer.last_focused` (`layer.rs`), and the matching note in `same_shape_layer` (`registry.rs`) to "reserved; populated externally by the focus tracker once the wire-up exists." Filed follow-up `01KQSHDPHBVM1RFD863SY6CCR9` to wire the kernel write path and tighten the docs back to active voice.

### Nits
- [x] `swissarmyhammer-focus/src/navigate.rs:350` — `reg.has_children(&cand.fq)` is called once per beam-search candidate inside `geometric_pick`, and `has_children` itself iterates the full `scopes` map (`children_of` filters `self.scopes.values()`). That makes `geometric_pick` O(N²) over scopes-in-layer, where the pre-collapse `is_zone()` was O(1). Production layouts have ≤ a few hundred scopes per layer so this is benign, but if a parent-children index is added later this loop is the call site that wants it. Optionally cache the boolean once-per-layer at the start of `geometric_pick` (a `HashSet<FullyQualifiedMoniker>` of "FQMs that appear as some scope's `parent_zone`") to restore O(N).
  - **Resolution**: `geometric_pick` now builds a `HashSet<&FullyQualifiedMoniker>` of in-layer parent_zone FQMs once per pick, and the candidate-loop tie-break checks `parent_fqs.contains(&cand.fq)` instead of calling `reg.has_children`. Restores O(N) over scopes-in-layer.
- [x] `swissarmyhammer-focus/tests/focus_registry.rs:14-23` — Module-doc preamble still references the deleted typed accessors (`scope`, `zone`), `children_of_zone`, `leaves_in_layer` / `zones_in_layer`, and the `ChildScope` view as if they exist. Test bodies use the new APIs; the docstring is the only stale piece. Replace with a one-liner pointing at the unified `find_by_fq` / `scopes_in_layer` / `children_of` / `has_children` surface.
  - **Resolution**: Replaced the stale preamble with a short sentence pointing at the unified surface (`find_by_fq`, `scopes_in_layer`, `children_of`, `has_children`, plus the layer-forest ops).
- [x] `swissarmyhammer-focus/tests/fixtures/mod.rs:276,336,508,667` — Fixture builder docs still describe components as "a `<FocusZone>`" or "kind `Zone`" even though the builder now produces uniform `FocusScope` values. Consistent with the task's acknowledgment for `in_zone_any_kind_first.rs` ("name is now slightly anachronistic"), but extends to other fixtures that didn't get the same call-out. Sweep the references in the same pass that updates `README.md` (sub-task D).
  - **Resolution**: Reviewer explicitly deferred this to sub-task D's docs sweep; not touching the fixtures here per "stay strictly on what findings call out". The deferral is recorded in the finding text itself.
- [x] `swissarmyhammer-focus/tests/overlap_tracing.rs:271` and `swissarmyhammer-focus/tests/duplicate_fqm_silent_swap.rs:355` — Inline comments still mention the old "kind discriminators (one zone + one scope)" / "FocusZone/FocusScope collapse" framing. Fine to leave as historical context with one trailing line ("kept after the collapse — every scope is uniform now") so a future reader doesn't chase a kind that no longer exists.
  - **Resolution**: Appended the suggested trailing line ("Kept after the collapse — every scope is uniform now.") to both comment blocks.
- [x] `swissarmyhammer-focus/tests/in_zone_any_kind_first.rs` (filename) — Per the task description, kept as-is intentionally; flagging only so it gets a line in sub-task D's docs sweep when the README is updated. A follow-up rename to e.g. `in_zone_siblings_first.rs` would align the filename with the new sibling-only model.
  - **Resolution**: Reviewer explicitly deferred to sub-task D; kept as-is per the task's existing call-out.