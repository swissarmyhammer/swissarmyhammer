---
assignees:
- claude-code
depends_on:
- 01KQSDP4ZJY5ERAJ68TFPVFRRE
position_column: todo
position_ordinal: d080
project: spatial-nav
title: 'Spatial-nav follow-up A: collapse FocusZone in kernel (scope/registry/navigate/lib)'
---
## Reference

Parent: `01KQSDP4ZJY5ERAJ68TFPVFRRE` — single-PR collapse was measured at 143 source files affected and split into 4 sequential sub-tasks (A/B/C/D). This is sub-task **A**.

After this task lands, the kernel crate compiles standalone with `FocusScope` as the only primitive. Other crates (`kanban-app`, `kanban-app/ui`) intentionally break — sub-task B fixes the IPC bridge, C sweeps React components, D sweeps tests + docs.

## What

Pure refactor inside `swissarmyhammer-focus` only. No behaviour change; the geometric algorithm and drill / first / last operations already don't distinguish kind — this just removes the type-level distinction.

### Files to modify

- `swissarmyhammer-focus/src/scope.rs`
  - Delete the `FocusZone` struct.
  - Collapse `RegisteredScope` (currently distinguishes scopes from zones) to one shape — likely a struct alias for `FocusScope` directly.
  - Move per-zone fields onto `FocusScope`: `last_focused: Option<FullyQualifiedMoniker>` (defaults to `None`; only populated when a child scope is focused), `show_focus_bar: bool` (already a field), and any others currently on `FocusZone`.
  - Drop `is_zone()` accessor — replaced by registry-level "has children" query.
  - Drop `ScopeKind::{Scope, Zone}`, `ChildScope::{Leaf, Zone}`, `FocusEntry::{Leaf, Zone}` enum variants. Each becomes a single shape.
  - Drop `BatchRegisterError::KindMismatch` — the variant becomes vacuous after the collapse.

- `swissarmyhammer-focus/src/registry.rs`
  - Single internal collection (not separate `zones`/`leaves`). Rewrite or rename whichever helper lives today (`zones_iter`, `leaves_iter`, `zones_in_layer`, `leaves_in_layer`, `zone()` lookup) to operate over the single collection.
  - Add `pub(crate) fn children_of(&self, parent_fq: &FullyQualifiedMoniker) -> impl Iterator<Item = &FocusScope>` — already exists in spirit as `child_entries_of_zone`; rename and generalize so any scope can have children.
  - Collapse `register_scope` and `register_zone` IPC handlers into a single `register_scope`. The wire-format `RegisterEntry` enum's `kind` discriminator is deleted (per approved decision: IPC is intra-process, no external consumers).
  - Delete the entire `scope-not-leaf` validation path (~150 LOC): `warn_scope_not_leaf`, `warn_existing_children_of_scope`, `is_path_descendant`, `same_shape` kind comparison. These become vacuous because the kind distinction no longer exists.
  - Confirm `first_child_by_top_left` / `last_child_by_bottom_right` (the helpers extracted under task `01KQQTZ7PSXEQF1WWX14ST8WRT`) still work — they don't depend on `is_zone()`.

- `swissarmyhammer-focus/src/navigate.rs`
  - The leaf-tie-break inside `geometric_pick` currently uses `is_zone()` to prefer leaves on equal scores. Rewrite as "prefer scopes with no registered children" using the new `children_of` query (or equivalent `has_children(fq)`).
  - Confirm `drill_in` / `drill_out` / `edge_command` (First / Last / RowStart / RowEnd) do not depend on the kind enum after the collapse — they all walk children-by-parent_zone.

- `swissarmyhammer-focus/src/lib.rs`
  - Sweep `pub use` exports — remove `FocusZone`, `RegisteredScope::Zone`, etc. Keep `FocusScope` as the public name.
  - Update doc-comment prose at the crate root to reflect the single-primitive model.
  - Update the `spatial_navigate` / `spatial_register_scope` doc-strings.

- `swissarmyhammer-focus/src/types.rs`
  - Sweep stale references in `Direction` / `Pixels` doc-comments.

### Files to update (kernel-internal tests)

These ARE part of this sub-task because they're inside the focus crate and break the build if not updated:

- `swissarmyhammer-focus/src/registry.rs::tests` (in-module)
- `swissarmyhammer-focus/src/navigate.rs::tests` (in-module)
- `swissarmyhammer-focus/src/scope.rs::tests` (in-module, if any)
- `swissarmyhammer-focus/tests/*.rs` — every integration test, including:
  - `cross_zone_geometric_nav.rs`
  - `coordinate_invariants.rs`
  - `card_directional_nav.rs`
  - `column_header_arrow_nav.rs`
  - `drill.rs`
  - `inspector_dismiss.rs`
  - `inspector_field_nav.rs`
  - `in_zone_any_kind_first.rs` (this one explicitly asserts kind distinction — reframe as "has children" / "no children" or delete if vacuous)
  - `navbar_arrow_nav.rs`
  - `navigate.rs`
  - `no_silent_none.rs`
  - `perspective_bar_arrow_nav.rs`
  - `unified_trajectories.rs`
- `swissarmyhammer-focus/tests/fixtures/mod.rs` — fixture builders shift from "register a scope or register a zone" to "register a scope; whether it's a leaf or container is determined by whether anything else registers under it."

### Out of scope for this sub-task

- Tauri IPC bridge (`kanban-app/src/commands.rs`) — sub-task B.
- React adapter (`spatial-focus-context.tsx`, `use-track-rect-on-ancestor-scroll.ts`) — sub-task B.
- React component callsites (`<FocusZone>` → `<FocusScope>`) — sub-task C.
- React-side test sweep, README rewrite — sub-task D.

## Acceptance Criteria

- [ ] `FocusZone` struct is deleted from `swissarmyhammer-focus/src/scope.rs`.
- [ ] `is_zone()` accessor is gone; replaced by `children_of(fq).next().is_some()` or equivalent at every callsite.
- [ ] `ScopeKind::Zone`, `ChildScope::Zone`, `FocusEntry::Zone`, `BatchRegisterError::KindMismatch` enum variants are gone.
- [ ] `register_zone` IPC handler is gone — `register_scope` is the only registration path.
- [ ] The `scope-not-leaf` validation path (`warn_scope_not_leaf`, `warn_existing_children_of_scope`, `is_path_descendant`, `same_shape`) is deleted.
- [ ] `geometric_pick`'s leaf-tie-break uses "no registered children" instead of `is_zone()`.
- [ ] `cargo test -p swissarmyhammer-focus` passes — all kernel-internal tests updated and green.
- [ ] `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` is clean.
- [ ] `grep -r "FocusZone\|is_zone\|register_zone\|ScopeKind::Zone\|BatchRegisterError::KindMismatch" swissarmyhammer-focus/src` returns no source-code matches.
- [ ] `cargo build --workspace` is INTENTIONALLY broken in `kanban-app` and `kanban-app/ui` build steps — sub-task B picks them up. The kernel itself compiles cleanly.

## Tests

- [ ] All existing `swissarmyhammer-focus/tests/*.rs` integration tests pass after the rename sweep — same behaviour, same assertions where they don't reference the deleted kind distinction.
- [ ] The four cross-zone regressions in `tests/cross_zone_geometric_nav.rs` still pass.
- [ ] The drill / first / last assertions in `tests/drill.rs` still pass.
- [ ] `tests/in_zone_any_kind_first.rs` — either reframe its kind-asserting tests as "has children" / "no children", or if the test premise depended entirely on the kind distinction, delete with a commit-message rationale.
- [ ] `cargo test -p swissarmyhammer-focus`: zero failures, zero warnings.

## Workflow

- This sub-task is a mechanical rename + delete inside one crate. The existing test suite is the safety net; no `/tdd` needed.
- Build incrementally: collapse the structs first, fix `cargo check -p swissarmyhammer-focus`, then update the kernel-internal tests, then run the full focus test suite.
- Do NOT touch `kanban-app/` or `kanban-app/ui/` — sub-tasks B/C/D handle those.
- If you find a callsite that does something genuinely zone-specific that's hard to translate, STOP and report — do not improvise.
#spatial-nav-redesign