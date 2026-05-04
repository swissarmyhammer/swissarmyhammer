---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8680
project: spatial-nav
title: 'Spatial-nav: mark Direction::RowStart / RowEnd #[deprecated] after #5 lands'
---
## Reference

Follow-up filed during review of spatial-nav #4 (`01KQQTZ7PSXEQF1WWX14ST8WRT`). Nit #2 from that review:

> `swissarmyhammer-focus/src/types.rs:130-135` and `src/types.rs:167-172` — The `RowStart`/`RowEnd` doc paragraph correctly explains they are aliases, but neither variant carries a `#[deprecated(note = "use Direction::First / Direction::Last")]` attribute. The implementer's rationale (TS side still references them, task #5 in flight) means a `#[deprecated]` attribute now would surface noise on every callsite — this is the right call for now. Recommend filing a follow-up task: "After spatial-nav #5 lands, add `#[deprecated]` to `Direction::RowStart` / `Direction::RowEnd` and migrate any remaining callsites to `First` / `Last`."

## What

Once spatial-nav #5 (the TypeScript-side `Direction` union migration) has landed and no Rust or TS callsite still names `Direction::RowStart` / `Direction::RowEnd`:

- `swissarmyhammer-focus/src/types.rs`: add `#[deprecated(note = "use Direction::First")]` to `Direction::RowStart` and `#[deprecated(note = "use Direction::Last")]` to `Direction::RowEnd`.
- Migrate any remaining Rust callsites that still name the aliased variants to `Direction::First` / `Direction::Last`.
- If TS-side references have also moved to `first` / `last`, consider whether the variants can be removed outright in a follow-up.

## Acceptance Criteria

- [x] `cargo build -p swissarmyhammer-focus --all-targets` produces no `deprecated` warnings (i.e. no callsite still uses the aliases).
- [x] `cargo clippy --all-targets -- -D warnings` clean.
- [x] All tests pass.

## Implementation notes (2026-05-03)

- Rust definition (`swissarmyhammer-focus/src/types.rs`): `RowStart` / `RowEnd` now carry `#[deprecated(note = "use Direction::First")]` and `#[deprecated(note = "use Direction::Last")]`. `Display` impl arms for the two variants carry `#[allow(deprecated)]` because `Display` is exhaustive on `Self` and external wire consumers may still send the strings during the deprecation window.
- Rust impl (`swissarmyhammer-focus/src/navigate.rs`): the four match sites that name the variants (`BeamNavStrategy::next`, `in_strict_half_plane`, `edge_command`, `score_candidate`) carry `#[allow(deprecated)]` on the function or surrounding match. The variants still route to the same `first_child_by_top_left` / `last_child_by_bottom_right` arms as `First` / `Last`.
- Rust tests:
  - In-module `row_start_end_are_aliases_for_first_last` was rewritten as `deprecated_row_start_end_still_alias_first_last` with `#[allow(deprecated)]`. It now compares the deprecated alias's result to the canonical `First` / `Last` result, pinning the alias-equivalence contract for the duration of the deprecation window.
  - Three integration tests in `tests/navigate.rs` (`row_start_alias_picks_leftmost_topmost_child`, `row_end_alias_picks_rightmost_bottommost_child`, `row_start_on_leaf_returns_focused_self`) were deleted — they duplicated coverage already pinned by the canonical `First` / `Last` integration tests, and the alias-preservation contract is covered by the renamed in-module test above.
- TypeScript (`kanban-app/ui/src/types/spatial.ts`): `"rowstart"` and `"rowend"` were dropped from the `Direction` string-literal union. The Rust kernel keeps the variants behind `#[deprecated]` for one release for wire-format consumers; the TS side jumps straight to the post-deprecation surface because no TS callsite ever named the strings (only the union declared them).
- Doc-comment updates in `src/lib.rs`, `src/navigate.rs`, `src/registry.rs`, `src/types.rs`, and `README.md` re-state the aliases as **deprecated** and replace `[`Direction::RowStart`]` doc-link references with plain prose so the docs no longer surface deprecation warnings on every link expansion.

### Verification commands run (all green)

- `cargo build -p swissarmyhammer-focus --all-targets` — Finished, zero warnings.
- `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` — Finished, zero warnings.
- `cargo clippy --all-targets -- -D warnings` (full workspace) — Finished, zero warnings.
- `cargo nextest run -p swissarmyhammer-focus` — 255 / 255 passed.
- `cargo nextest run` (full workspace) — 13615 / 13615 passed (5 skipped, pre-existing).
- `pnpm -C kanban-app/ui exec tsc --noEmit` — Clean.
- `pnpm -C kanban-app/ui test --run` — 1985 / 1985 passed (1 skipped, pre-existing) across 206 test files.

### Followup deferred

The task description's third bullet ("If TS-side references have also moved to `first` / `last`, consider whether the variants can be removed outright in a follow-up") is not actioned here — outright removal would break wire-format compatibility for any external consumer mid-deprecation, and the design intent of `#[deprecated]` is exactly to give one release of warning before removal. A follow-up task can be filed once the deprecation window has elapsed and no `RowStart` / `RowEnd` traffic is observed on the wire.

## Workflow

- Verify spatial-nav #5 has landed before starting (this task is gated on it).
- Use grep / code_context to enumerate remaining `RowStart` / `RowEnd` references; migrate before adding the attribute so the build stays clean.
#spatial-nav-redesign

## Review Findings (2026-05-03 18:30)

### Nits
- [x] `kanban-app/ui/src/lib/scroll-on-edge.ts:31, 67, 216, 296-297` — Doc comments still list `"rowstart"` / `"rowend"` alongside `"first"` / `"last"` as if they were valid `Direction` literals. The TS `Direction` union dropped them in this same change, so these mentions are stale and contradict the type definition. Suggest replacing each occurrence with `first` / `last` only (e.g. line 31: "`first` / `last` are not cardinal in the geometric sense"; line 67: "Non-cardinal values (`first`, `last`) return `null`...").
- [x] `kanban-app/ui/src/components/grid-view.spatial-nav.test.tsx:652` — Comment references `nav.rowstart` / `nav.rowend` keymap commands that do not exist (only `nav.first` / `nav.last` are registered in `app-shell.tsx` / `keybindings.ts`). Drop those two from the keymap comment so it matches the post-redesign surface.
- [x] `swissarmyhammer-focus/README.md:321` — Quotes the deprecation note as `"use Direction::First / Direction::Last"`, but the actual attributes split into two distinct notes (`"use Direction::First"` on `RowStart`, `"use Direction::Last"` on `RowEnd`). Either rewrite the README sentence to describe the per-variant attributes (e.g. "carry `#[deprecated(note = "use Direction::First")]` and `#[deprecated(note = "use Direction::Last")]` respectively") or unify the attributes — the former is closer to the existing prose.
- [x] `swissarmyhammer-focus/src/types.rs:172, 178` — `#[deprecated]` attribute uses only `note`. Consider adding `since = "0.12.11"` (the current workspace version) so the deprecation version surfaces in rustdoc and downstream tooling. Optional but conventional; helps consumers tracking the one-release migration window.

### Resolution (2026-05-04)

- Nit 1 (scroll-on-edge.ts): Removed `rowstart` / `rowend` from the four doc comments at the file header, `axisFor`, `isCardinal`, and `runNavWithScrollOnEdge` step 2.
- Nit 2 (grid-view.spatial-nav.test.tsx): Rewrote the keymap comment to drop `nav.rowstart` / `nav.rowend` and the `PageUp / PageDown` parenthetical (which were also stale — those keys are not bound).
- Nit 3 (README.md): Rewrote the deprecation paragraph to describe the per-variant attributes, also reflecting the `since = "0.12.11"` field added in nit 4.
- Nit 4 (types.rs): Added `since = "0.12.11"` to both `#[deprecated]` attributes on `Direction::RowStart` and `Direction::RowEnd`.

### Verification (2026-05-04)

- `cargo nextest run -p swissarmyhammer-focus` — 255 / 255 passed.
- `cargo nextest run` (full workspace) — 13615 / 13615 passed (5 skipped, pre-existing).
- `cargo clippy --all-targets -- -D warnings` (full workspace) — Finished, zero warnings.
- `pnpm -C kanban-app/ui exec tsc --noEmit` — Clean.
- `pnpm -C kanban-app/ui test --run` — 1985 / 1985 passed (1 skipped, pre-existing) across 206 test files.