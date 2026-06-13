---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa780
title: ui-state.yaml can be clobbered with defaults — load failure or restart churn must never overwrite user settings
---
## What

LIVE DATA LOSS (user-observed 2026-06-12): after an afternoon of `tauri dev` restart churn (a dozen restarts in 3 minutes during Rust edits), the user's persisted settings degraded: keymap_mode lost (vim → default cua) and open_boards shrank (4 → 2; one was the legitimately-pruned malformed board, but `swissarmyhammer-kanban/.kanban` also fell out).

## Suspected mechanism

`UIState::load` (crates/swissarmyhammer-ui-state/src/state.rs:398) + `save` (:437) on `$XDG_CONFIG_HOME/sah/kanban-app/ui-state.yaml`. Every app start loads the file and subsequent state changes save the WHOLE file. If a load fails (parse error, torn read during a racing save from the dying previous instance) the state silently falls back to defaults — and the next save writes those defaults over the user's file. Rapid restart cycles (tauri dev rebuild loop) make the read-during-write race likely.

## Investigate
1. Read UIState::load — what happens on a missing/corrupt/partial file? (silent default?) Is there any logging?
2. Read save — atomic (write temp + rename) or in-place truncate? In-place truncate + a restart mid-write = torn file for the next loader.
3. Check open_boards shrink: is there any path that drops entries other than restore-prune (path not a dir)? Did the failed-open path remove entries (it shouldn't have per the code read earlier — failed opens keep entries)?

## Investigation findings (2026-06-12)

1. `read_from_file` parse failure → `tracing::warn!` + silent in-memory defaults; nothing preserved. The next auto-save (any mutation — e.g. `add_open_board` during startup board discovery) wrote those defaults over the user's file.
2. `save` used `std::fs::write` — in-place truncate-then-write, NOT atomic. `main.rs` `handle_run_event` also saves unconditionally on every exit, so each dev-loop restart rewrote the file, maximizing the torn-read window. The torn read was reproduced live in a test: a concurrent loader observed `keymap_mode: cua` (defaults) while a saver was mid-write.
3. open_boards drop paths: only restore-prune (`restore_persisted_boards`, removes non-dir/empty paths), and explicit board close (`commands.rs` board.close / `state.rs` close path). Failed opens keep entries. `swissarmyhammer-kanban/.kanban` is a real directory, so it could NOT have dropped via prune — the defaults-clobber in (1)+(2) is the mechanism, and it explains the keymap loss and the boards loss as one event (the surviving 2 boards were the ones re-opened by discovery/window-restore in the clobbering session).

Bonus: the pre-existing `load_malformed_yaml_returns_defaults` test used `:::not valid yaml:::`, which actually PARSES as a YAML mapping with unknown keys (serde ignores them) — the parse-failure path was never really tested.

## Required outcome
- **Parse-or-preserve**: a failed/partial load NEVER leads to defaults being saved over the existing file. On load failure: keep the file untouched, log loudly, optionally load defaults in-memory but mark the state non-persisting until a successful explicit user change (or back the old file up as ui-state.yaml.corrupt-<ts> before any overwrite).
- **Atomic writes**: save via temp file + rename so a mid-write kill never leaves a torn file.
- **No save-on-load**: starting the app with no changes must not rewrite the file at all.

## Implemented (crates/swissarmyhammer-ui-state/src/state.rs only; no app-side changes needed)

- Parse failure → `tracing::error!` + copy original bytes to `<name>.corrupt-<unix-secs>` sibling before defaults are allowed; if the backup fails, persistence is blocked for the instance lifetime.
- Read failure (non-NotFound, e.g. EACCES) → defaults in memory, persistence blocked (file we never saw is never overwritten).
- `save` writes a `.{name}.tmp-<pid>-<n>` sibling then `rename`s it into place (atomic, cleanup on rename failure), serialized under a lock.
- `save` keeps a serialized baseline from load/last save and skips the write entirely when state is unchanged — the app's unconditional save-on-exit no longer touches the file after a change-free session.

## Acceptance Criteria
- [x] Corrupt/truncated ui-state.yaml at startup: file preserved (or backed up), warn logged, app runs with in-memory defaults
- [x] Kill -9 during save: file is either old or new content, never torn (atomic rename)
- [x] Clean start + clean exit with no setting changes: file byte-identical
- [x] keymap_mode and open_boards survive a 10× rapid restart loop (test simulates load/save cycles)

## Tests
- [x] Rust unit/integration tests in swissarmyhammer-ui-state for each criterion (red-first where current behavior fails): `corrupt_config_file_preserved_and_backed_up_on_load`, `mutation_after_corrupt_load_keeps_backup_of_original`, `unreadable_config_blocks_auto_save_clobber`, `save_after_load_with_no_changes_does_not_touch_file` (bytes + inode + mtime), `save_replaces_file_atomically_via_rename`, `settings_survive_rapid_restart_cycles`, `concurrent_load_during_save_never_sees_torn_state` (reproduced the live torn-read red-first)
- [x] cargo nextest -p swissarmyhammer-ui-state green — 143/143 (baseline was 136/136)

## Follow-up

- 01KTYYZFP54886NBEMXTGT50A6 — consolidate the workspace's three hand-rolled atomic-write helpers into swissarmyhammer-common::fs_utils (review finding; cross-crate, out of this card's scope).

## Constraints
- NO whole-workspace builds; crate-scoped only. Never touch .kanban/actors/wballard.jsonl.

## Workflow
- /tdd.