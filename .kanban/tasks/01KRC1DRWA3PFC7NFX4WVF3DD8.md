---
assignees:
- claude-code
depends_on:
- 01KRC1C93CD73746F4C0Q2PP86
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd280
title: Switch backend perspective filters to view_id with kind fallback
---
## What

Now that `Perspective.view_id: Option<String>` exists (preceding task), update every backend filter that currently matches by `view_kind` to prefer `view_id` when present and fall back to `view` (kind) match only for legacy perspectives where `view_id == None`.

### Files to modify

- `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — every site listed below currently filters `p.view == view_kind`. Replace each with the helper:
  ```rust
  fn perspective_belongs_to_active_view(
      p: &Perspective,
      active_view_id: Option<&str>,
      active_view_kind: &str,
  ) -> bool {
      match (&p.view_id, active_view_id) {
          (Some(pid), Some(active)) => pid == active,    // strict id match
          (None, _) => p.view == active_view_kind,        // legacy shared-by-kind
          (Some(_), None) => false,                       // perspective is scoped but caller has no view id
      }
  }
  ```
  Sites to convert (line numbers from current HEAD — re-grep before editing): ~115, ~123, ~300, ~331, ~354, ~382, ~559, ~578, ~639, ~666, plus the closure inside `goto_perspective` (~727).
- `swissarmyhammer-kanban/src/dynamic_sources.rs:gather_perspectives` — change signature to take both `active_view_id: Option<&str>` and `active_view_kind: Option<&str>`. Inside, filter using the helper. Update the one call site at `build_dynamic_sources` (~108–113) to compute both from `resolve_active_view_kind` (currently returns only kind — extend it to also return the active view id, e.g. `resolve_active_view(...) -> Option<(String, String)>` returning `(id, kind)`, or split into two helpers `resolve_active_view_id` + `resolve_active_view_kind`).
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs:resolve_view_kind` — extend the resolver so callers can also obtain the active view id. Either return a tuple or add a sibling `resolve_view_id` helper. Update both `next_perspective` and `cycle_perspective` and `goto_perspective` to thread the view id through.
- `swissarmyhammer-kanban/builtin/commands/perspective.yaml` — already declares a `view_id` arg from the preceding task. Ensure the verbs that resolve perspectives (`next`, `cycle`, `goto`) declare `view_id` as optional alongside `view_kind`.

### Behavior

- New scoped perspectives (those with `view_id`) appear ONLY in the view whose id matches — switching between two grid views with different ids shows different perspective sets.
- Legacy `view_id == None` perspectives keep their pre-existing "shared by kind" behavior so existing files do not lose visibility.
- When the caller cannot resolve an active view id (e.g. headless dynamic-sources before a view is focused), only legacy perspectives appear — scoped ones do not "leak" globally.

### Out of scope

- Frontend filter swap — see "Switch frontend perspective tab bar filter to view_id with kind fallback".
- Re-saving existing YAMLs — see "Migrate existing perspective YAMLs to carry view_id where unambiguous".

## Acceptance Criteria

- [x] All ~10 backend filter sites enumerated above use the `perspective_belongs_to_active_view` helper (no remaining `p.view == view_kind` filter expressions for perspectives).
- [x] `gather_perspectives` signature reflects the new resolver shape and the single call site is updated.
- [x] `resolve_view_kind` (or its replacement) exposes the active view id to callers.
- [x] Behavior matrix is verified by tests:
  - perspective with `view_id` set to view A's id, active view is A → returned
  - perspective with `view_id` set to view A's id, active view is B (same kind) → NOT returned
  - legacy perspective (`view_id: None`, `view: "grid"`), active view is any grid → returned
  - legacy perspective (`view_id: None`, `view: "grid"`), active view is board → NOT returned
- [x] `cargo test -p swissarmyhammer-kanban` passes including new tests.

## Tests

- [x] Integration test in `swissarmyhammer-kanban/tests/dynamic_sources_headless.rs`: `perspectives_are_scoped_by_view_id_when_set` — register two grid-kind views (`view_a` id `01JMVIEW0000000000TGRID0`, `view_b` id `01JMVIEW0000000000PGRID0`), create a perspective with `view: "grid"`, `view_id: Some(view_a_id)`. Build dynamic sources with `view_b` active, assert the perspective is NOT in the result. Switch to `view_a` active, assert it IS in the result.
- [x] Integration test: `legacy_kind_perspectives_remain_shared_by_kind` — perspective with `view: "grid"`, `view_id: None`. Both grid views return it in `gather_perspectives`; board view does NOT.
- [x] Unit tests in `swissarmyhammer-kanban/src/commands/perspective_commands.rs` mod-tests:
  - `next_perspective_filters_by_view_id_when_arg_provided` — add `view_id` arg to existing fixture, assert only the matching perspective is in the candidate set.
  - `next_perspective_falls_back_to_legacy_perspectives_when_view_id_absent` — perspective with `view_id: None`, kind-only filter still selects it.
- [x] Update any existing tests that constructed `Perspective` with only `view` to also pass `view_id: None` explicitly so the test reads clearly. (Note: `Perspective::new` constructor sets `view_id: None` implicitly; existing tests already read clearly and were left unchanged. The new helper-unit tests pass `None` explicitly via `make_perspective`.)
- [x] Run: `cargo test -p swissarmyhammer-kanban` — green.

## Workflow

- Use `/tdd` — write the new dynamic-sources scoping tests first, watch them fail (current behavior leaks across grid views), then implement the filter swap.
- Re-grep line numbers immediately before editing — they drift.
- Use `code_context get blastradius` on `perspective_commands.rs` and `dynamic_sources.rs` before editing to surface any callers that pass `view_kind` and now need a `view_id`. #perspective-view-id