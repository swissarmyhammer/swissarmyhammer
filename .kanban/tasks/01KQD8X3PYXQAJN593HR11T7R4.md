---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffe080
project: spatial-nav
title: 'Path monikers Layer 1: Rust newtypes + kernel registry FQM rewire (cargo test green)'
---
## Subset of `01KQD6064G1C1RAXDFPJVT1F46`

This is the first of three sequenced sub-tasks decomposed from the omnibus path-monikers card. The omnibus card touches ~1284 callsites across Rust + TypeScript and requires macOS `log show` verification — too large for a single `/implement` pass. This subset is bounded by `cargo test -p swissarmyhammer-focus` going green.

## What

- Introduce `SegmentMoniker(String)` and `FullyQualifiedMoniker(String)` newtypes in `swissarmyhammer-focus/src/types.rs`. Distinct types — `find_by_fq(SegmentMoniker)` must not compile.
- `FullyQualifiedMoniker::compose(parent: &FullyQualifiedMoniker, segment: &SegmentMoniker) -> FullyQualifiedMoniker` — appends with `/` separator.
- `FullyQualifiedMoniker::root(segment) -> FullyQualifiedMoniker` (or equivalent) for layer roots.
- Replace `SpatialKey` with `FullyQualifiedMoniker` as the registry key. `RegisteredScope` carries `fq: FullyQualifiedMoniker` + `segment: SegmentMoniker` (for human logs only). Delete `SpatialKey` from the kernel.
- Keep `Moniker` (flat) gone — kernel surface uses only `SegmentMoniker` and `FullyQualifiedMoniker`.
- Rewire `register_zone`, `register_scope`, `unregister_scope`, `update_rect`, `find_by_fq`, `drill_in`, `drill_out`, `apply_batch`, `RegisterEntry` etc. to take FQM as the identifier.
- Rewire `SpatialState::focus`, `focus_by_moniker` → `focus(&FullyQualifiedMoniker)`, `clear_focus`, `handle_unregister`, `resolve_fallback`, `navigate_with` to FQM.
- Rewire `BeamNavStrategy` to identify candidates by FQM.
- Keep `FocusChangedEvent` shape: `{ window_label, prev_fq, next_fq, next_segment }`.
- Update existing kernel tests + integration tests in `swissarmyhammer-focus/tests/*.rs` to use FQM (compile-error wave guides).

## Acceptance Criteria

- [x] `SegmentMoniker` and `FullyQualifiedMoniker` are distinct newtypes (no `String` aliases).
- [x] `SpatialKey` (UUID) deleted from `swissarmyhammer-focus`.
- [x] `find_by_fq` is the only lookup-by-identifier API; takes `&FullyQualifiedMoniker`.
- [x] New file `swissarmyhammer-focus/tests/path_monikers.rs` contains the six named Layer 1 tests from the parent card.
- [x] `cargo test -p swissarmyhammer-focus` passes.
- [x] `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` clean.

## Out of scope (handled in follow-up cards)

- Tauri commands (kanban-app/src/commands.rs).
- React adapter (FocusLayer/Zone/Scope), entity-focus-context, all UI tests.
- Layer 3 manual log verification (`npm run tauri dev`).

These will block on this sub-task landing first.

## Related

- Parent: `01KQD6064G1C1RAXDFPJVT1F46`
- Cross-ref: `01KQAW97R9XTCNR1PJAWYSKBC7` (no-silent-dropout)

## Review Findings (2026-04-30 14:42)

Clean — zero findings. Verified by `/review`:

- All six named Layer 1 tests are present in `swissarmyhammer-focus/tests/path_monikers.rs` and pass: `register_zone_keyed_by_fq_moniker`, `two_zones_same_segment_different_layers_have_distinct_fq_keys`, `find_by_fq_unknown_path_returns_none_and_traces_error`, `cascade_does_not_cross_layers`, `segment_moniker_does_not_compile_at_fq_lookup_callsite`, `register_with_duplicate_fq_logs_error_and_replaces`.
- `cargo test -p swissarmyhammer-focus` passes (path_monikers 6/6, perspective_bar_arrow_nav 5/5, traits_object_safe 5/5, unified_trajectories 4/4, doc-tests 1/1, all other suites green).
- `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` clean.
- `SpatialKey` only appears in historical-context doc comments (no source uses).
- Every kernel API (`register_zone`, `unregister_scope`, `update_rect`, `scope`, `zone`, `layer`, `is_registered`, `ancestor_zones`, `remove_layer`, `children_of_layer`, `ancestors_of_layer`, `drill_in`, `drill_out`, `apply_batch`, `SpatialState::focus`, `clear_focus`, `handle_unregister`, `focused_in`) takes `&FullyQualifiedMoniker`.
- Newtypes built via `define_id!` (`#[serde(transparent)]`, `Display`, `AsRef<str>`, `FromStr`, etc.) match the workspace ID-newtype pattern. `FullyQualifiedMoniker::root` and `compose` are the well-formed constructors; `FQ_SEPARATOR = '/'` matches the React-side composition.

Acceptance-criteria boxes flipped on this review.
