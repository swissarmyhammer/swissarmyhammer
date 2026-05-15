---
assignees:
- claude-code
depends_on:
- 01KRC1DRWA3PFC7NFX4WVF3DD8
- 01KRC1F2D259GQDN83M1YVPX0R
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd380
title: Migrate existing perspective YAMLs to carry view_id where unambiguous
---
## What

Close out the per-view-id scoping epic by upgrading existing on-disk perspectives so they stop relying on the legacy shared-by-kind fallback whenever the assignment is unambiguous. After this lands, the typical `.kanban/perspectives/*.yaml` carries a `view_id` and behaves correctly under per-view scoping; only perspectives with genuinely ambiguous kind-matches (multiple views of the same kind) are left as legacy and surfaced to the user.

### Migration policy

Implement at perspective-load time inside `swissarmyhammer-kanban/src/perspective/` (locate the loader via `code_context get symbol load_perspective` / `Perspective::load`):

1. **Unambiguous kind → auto-assign on next save.** When a perspective with `view_id: None` is loaded and the workspace currently has exactly one view whose `kind` matches the perspective's `view`, the loader records the mapping in memory and the next `UpdatePerspective` / re-save writes `view_id` back to disk. Do NOT rewrite YAMLs on read — only on write — so the migration is opt-in and non-destructive.
2. **Ambiguous kind → leave as legacy.** When multiple views of the same kind exist (e.g. three `kind: grid` views in this repo: `01JMVIEW0000000000TGRID0`, `01JMVIEW0000000000PGRID0`, `01JMVIEW0000000000TGGRD0`), the perspective stays in legacy shared-by-kind mode. Log a one-time `info!` message: `"perspective <id> remains shared across all <kind> views — open it in a specific view and save to pin it"`.
3. **No matching view at all** → behave as today (perspective is effectively orphaned); log a `warn!` once.

### Files to modify

- `swissarmyhammer-kanban/src/perspective/` — add a `migrate_view_id` helper that runs during load OR a `maybe_pin_view_id_on_save` helper that runs during save. Pick one; document the choice in the helper's doc-comment.
- `swissarmyhammer-kanban/src/commands/perspective_commands.rs` — `UpdatePerspective` (and any other save path) calls the helper before writing.
- `swissarmyhammer-kanban/src/dynamic_sources.rs` or wherever the views context is exposed — make sure the helper can read the current view set so it can detect "exactly one view of this kind".
- `swissarmyhammer-kanban/builtin/commands/perspective.yaml` — if any verb description references "shared by view kind", update it to describe the new per-id semantics with the legacy fallback called out.

### Out of scope

- Touching the seven existing `.kanban/perspectives/*.yaml` files in this repo directly. Migration must work for any consumer's existing files, not just ours.

## Acceptance Criteria

- [x] After this task, a legacy perspective whose kind matches exactly one view in the workspace gets its `view_id` written to disk on the next save — verified by `legacy_perspective_unambiguous_kind_migrates_on_save` in `tests/perspective_migration.rs` (asserts the resulting YAML contains a `view_id:` line).
- [x] A legacy perspective whose kind matches multiple views (e.g. `view: grid` with 3 grid views present) is NOT auto-pinned on save unless the user explicitly opens it in a specific view and re-saves; the YAML stays untouched. Verified by `legacy_perspective_ambiguous_kind_stays_legacy`.
- [x] A one-time log line is emitted for each ambiguous legacy perspective at startup so the user knows why it didn't migrate. Verified by `legacy_perspective_ambiguous_emits_one_time_log` (uses `tracing-test::traced_test` + `logs_assert` to assert exactly one occurrence per perspective id).
- [x] `cargo test -p swissarmyhammer-kanban` passes (1124 lib unit tests + every integration test, including the new perspective_migration suite).
- [x] All four sub-tasks of the original epic are complete and the original behavior bug ("two grid views share one perspective pool") is gone — the existing backend regression tests `perspectives_are_scoped_by_view_id_when_set` and `legacy_kind_perspectives_remain_shared_by_kind` in `tests/dynamic_sources_headless.rs` still pass alongside the new migration suite. The frontend regression test from task 3 is already in place.

## Tests

- [x] Integration test in `swissarmyhammer-kanban/tests/perspective_migration.rs` (new file): `legacy_perspective_unambiguous_kind_migrates_on_save` — seeds a legacy YAML, registers exactly one `kind: list` view, dispatches `update perspective` and asserts the on-disk YAML now contains `view_id: <id>`.
- [x] Integration test: `legacy_perspective_ambiguous_kind_stays_legacy` — seeds a legacy `view: grid` YAML against the (already grid-ambiguous) builtin views, dispatches `update perspective`, asserts the YAML still has no `view_id` line.
- [x] Integration test: `legacy_perspective_ambiguous_emits_one_time_log` — uses `#[traced_test]` to capture logs, walks `build_dynamic_sources` twice over the same ambiguous perspective, asserts exactly one `info!` line referencing the perspective id is emitted.
- [x] `cargo test -p swissarmyhammer-kanban` is green end-to-end.

## Implementation Notes

- **Helper placement: save-time.** Chose `maybe_pin_view_id_on_save` in `swissarmyhammer-kanban/src/perspective/migrate.rs` rather than a load-time mapping. Rationale (documented in the module's doc-comment): the task spec explicitly requires opt-in, non-destructive migration (YAMLs untouched until next save), and `PerspectiveContext::open` in the perspectives crate has no access to `ViewsContext`. Only the command layer in this crate has both, so save-time wiring is the cleanest fit.
- **Wiring:** `AddPerspective`, `UpdatePerspective`, and `RenamePerspective` all call `maybe_pin_view_id_on_save` immediately before `pctx.write(&p).await`. `RenamePerspective` was refactored from `pctx.rename()` to a read-mutate-write-with-migration sequence so it benefits from the same migration path.
- **One-time logging:** `log_legacy_perspectives_once` runs inside `dynamic_sources::gather_perspectives`. A process-wide `Mutex<HashSet<perspective_id>>` guards repeat emissions. The test resets the guard via `perspective::migrate::reset_legacy_log_guard_for_test` (gated `#[cfg(any(test, feature = "test-support"))]`).
- **No `perspective.yaml` updates needed:** the verb descriptions never said "shared by view kind"; they always referred to view ids by name, so nothing to retitle.

## Workflow

- Use `/tdd` — write the migration unit tests first, then implement.
- This task depends on the data-shape, backend-filter, and frontend-filter tasks all being merged — sequence it last in the epic.
- Do NOT modify the seven existing `.kanban/perspectives/*.yaml` files in this repo manually. They will migrate organically on next save, which is the intended UX. (If a maintainer wants to migrate them eagerly, that's a one-line shell loop, not engineering work.) #perspective-view-id