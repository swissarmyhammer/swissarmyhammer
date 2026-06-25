---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw05pm4r55s3tb4ype9f657v
  text: |-
    Implemented the consolidation (TDD: wrote 4 failing write_atomic tests in fs_utils, watched RED, then GREEN).

    Helper added: `swissarmyhammer_common::fs_utils::write_atomic(path: &Path, contents: &str) -> std::io::Result<()>` — a free fn that creates the parent dir (guarded against empty parent), mints a unique temp sibling `.<file>.tmp-<pid>-<seq>` via a private `temp_sibling` helper + process-wide AtomicU64 counter, writes the temp, renames over the target, and cleans up the temp on EITHER write OR rename failure. No cfg(test) non-atomic escape hatch (the task's stated goal). It is plain std::fs; the trait-based StdFileSystem::write_with_permissions stays as-is for permission-aware callers (left untouched — flattening it would lose the permissions axis, exactly the parameterize-don't-flatten warning).

    Call sites migrated:
    1. crates/swissarmyhammer-ui-state/src/state.rs — UiState::save now calls write_atomic and the private temp_sibling was deleted. Added swissarmyhammer-common as a normal dep in that crate's Cargo.toml. The baseline-lock / change-skip / persist_blocked logic stays in save(); only temp+write+rename+create_dir moved to the helper.
    2. crates/swissarmyhammer-plugin/src/codegen.rs — TypesEmitter write path now calls write_atomic; the local atomic_write + temp_sibling fns were deleted. Promoted swissarmyhammer-common from dev-dep to a normal dep (codegen is production code). No dependency cycle (common does not depend on plugin/ui-state).

    Tests: cargo nextest GREEN for the touched behavior — 4 new common write_atomic tests, ui-state save_replaces_file_atomically_via_rename + concurrent_load_during_save_never_sees_torn_state + settings_survive_rapid_restart_cycles + save_creates_parent_directories + save_after_load_with_no_changes_does_not_touch_file, and plugin codegen::tests::write_is_atomic_and_leaves_no_temp_file all pass. cargo fmt + cargo clippy --all-targets -D warnings clean for all three crates.

    Pre-existing unrelated failures: the full `-p swissarmyhammer-plugin` run has 3 failures (file_notes_e2e::file_notes_plugin_round_trips_through_files_tool, example_layering_e2e::committed_examples_coload_across_layers, hot_reload::a_failed_v2_load... [flaky/timing]). Verified these fail identically on baseline with my changes stashed — they are NOT caused by this consolidation (none touch atomic-write; the e2e ones fail on a missing notes/hello.txt in a deno-runtime layering scenario).
  timestamp: 2026-06-25T19:56:52.248624+00:00
- actor: claude-code
  id: 01kw05w880dv0xybzcnbv0fdv3
  text: |-
    really-done: verification commands green (see prior comment); adversarial double-check returned PASS. Confirmed no behavior regression at either call site, no orphaned imports, no dependency cycle, write_with_permissions untouched, and the two pinned ui-state regression tests still meaningfully assert atomicity through the shared path.

    One advisory observation (accepted, not fixed — outside scope): codegen's temp-name format changed cosmetically from `.<file>.<pid>.<seq>.tmp` to `.<file>.tmp-<pid>-<seq>` (now matching ui-state's old format). As a side effect, the `name.ends_with(".tmp")` filter in codegen::tests::write_is_atomic_and_leaves_no_temp_file no longer matches the new name, making that one filter effectively dead. The test is NOT weakened: the immediately following `assert_eq!(entries, vec!["app.d.ts"])` asserts the directory contains exactly the destination and nothing else, fully covering "no temp litter". Optional future cleanup: update or drop the stale `.tmp` filter. Left as-is to keep the change scoped to the consolidation.

    Leaving task in `doing`, green, ready for /review.
  timestamp: 2026-06-25T19:59:56.672210+00:00
- actor: claude-code
  id: 01kw07ks6k0qrz0tvyhb915qeh
  text: |-
    Review nit addressed: changed `write_atomic` signature in crates/swissarmyhammer-common/src/fs_utils.rs from `path: &Path` to `path: impl AsRef<Path>` (added `let path = path.as_ref();` as the first body line) to match std::fs::write conventions and accept PathBuf/&str/&Path without conversion. Body otherwise unchanged (parent-dir create, temp_sibling, write, rename, cleanup).

    Note: could not flip a `- [ ]` checkbox — the task description has no findings checklist (only What / Proposed outcome / Constraints), and neither comment contains a checklist. Recording the resolution here instead.

    Verification (all green): `cargo check -p swissarmyhammer-common` exit 0; `cargo nextest run -p swissarmyhammer-common` 607/607 passed (incl. the 4 write_atomic tests); `cargo nextest run -p swissarmyhammer-ui-state -p swissarmyhammer-plugin` — ui-state save_replaces_file_atomically_via_rename + concurrent_load_during_save_never_sees_torn_state pass; the only 2 failures (file_notes_e2e, example_layering_e2e) are the documented pre-existing/unrelated ones, flaky hot_reload passed this run. `cargo fmt` + `cargo clippy -p swissarmyhammer-common --all-targets -- -D warnings` clean.

    Leaving task in `doing`, green.
  timestamp: 2026-06-25T20:30:16.275251+00:00
position_column: doing
position_ordinal: '8180'
title: Consolidate hand-rolled atomic-write (temp sibling + rename) helpers into swissarmyhammer-common::fs_utils
---
## What

Review of the ui-state clobber-protection fix (01KTYVRR39X1TFFTSH87X62QH1) found the workspace now has three independent copies of the atomic-write pattern (write temp sibling → rename → cleanup-on-rename-failure):

- `crates/swissarmyhammer-common/src/fs_utils.rs` — `StdFileSystem::write_with_permissions` (trait-shaped, `SwissArmyHammerError`, and a `#[cfg(test)]` branch that writes NON-atomically in common's own test builds)
- `crates/swissarmyhammer-ui-state/src/state.rs` — `UIState::save` + `temp_sibling` (io::Result, pid+counter temp names)
- a near-verbatim private `temp_sibling` in `codegen.rs` (per review output)

Each copy gets hardened independently (e.g. the fs_utils version sets permissions before rename; the others do not).

## Proposed outcome

- Hoist a free function `write_atomic(path, contents) -> io::Result<()>` (temp-sibling mint + write + rename + cleanup) into `swissarmyhammer-common::fs_utils`, without the cfg(test) non-atomic escape hatch.
- Migrate `UIState::save` (add the `swissarmyhammer-common` dependency to `swissarmyhammer-ui-state`) and `codegen.rs::temp_sibling` to it.
- Keep ui-state's atomicity regression tests (`save_replaces_file_atomically_via_rename`, `concurrent_load_during_save_never_sees_torn_state`) green — they pin the behavior across the migration.

## Constraints

- Crate-scoped builds/tests only (common, ui-state, codegen's crate).