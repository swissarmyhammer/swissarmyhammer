---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff9180
title: 'AVP: filter Stop validator inputs by content SHA against last-allowed-Stop baseline; unify changed-files lookups'
---
## What

When AVP runs validators on a Stop hook, every diff sidecar accumulated since the start of the Claude session is fed in. On a blocked Stop the agent iterates, but the next Stop re-evaluates the *full* cumulative set — files that have already been brought back to a known-good state are still re-validated. The user-visible symptom is "too many diffs per turn."

Fix this by remembering, at every **allowed** Stop, the SHA-256 of each currently-changed file's on-disk content, and using that baseline at the next validator run to skip any candidate file whose current content hashes to the same value.

Touch these files only:

- `avp-common/src/turn/state.rs` — extend `TurnState`, add hashing + baseline write API on `TurnStateManager`.
- `avp-common/src/strategy/claude/strategy.rs` — record baseline in `maybe_cleanup_turn_state` *before* the existing `clear()` calls.
- `avp-common/src/chain/links/validator_executor.rs` — unify the two Stop-time lookups behind one function and apply the SHA filter there.

### 1. Extend `TurnState` (state.rs)

Add one field, defaulting to empty so existing on-disk YAML files keep loading:

```rust
/// SHA-256 of each file's content as of the last allowed Stop.
/// Used to skip files in the next validator run whose content has not
/// changed since the previous allowed Stop. Cleared on SessionStart and
/// replaced wholesale on each allowed Stop.
#[serde(default)]
pub last_stop_shas: std::collections::HashMap<PathBuf, String>,
```

`TurnState::default()` already covers the empty map. No migration needed — `serde(default)` handles older YAML.

### 2. Baseline-recording API on `TurnStateManager` (state.rs)

Add a method (small, focused — well under 50 LOC):

```rust
/// Record the SHA-256 of each currently-changed file's on-disk content
/// as the new "last allowed Stop" baseline for `session_id`.
///
/// Replaces (does not merge) `state.last_stop_shas`. Files in `paths`
/// that no longer exist on disk are omitted from the baseline. Called
/// from the allowed-Stop cleanup path *before* `clear()` wipes the
/// rest of the state.
pub fn record_stop_baseline(
    &self,
    session_id: &str,
    paths: &[PathBuf],
) -> Result<(), AvpError>
```

Implementation: load state, build a fresh `HashMap`, hash each path's bytes via `sha2::Sha256`, hex-encode, insert. Save state back through the existing locked `save` path. Skip files that fail to read (log at `tracing::debug!` level — deletion is legitimate; treat as "not in baseline" so it falls through to validators next run). Hashing reuses the existing crypto dependency already pulled in by the workspace.

Also add a public read accessor used by the validator executor:

```rust
/// Hex-encoded SHA-256 of the file content recorded at the last
/// allowed Stop, if any.
pub fn last_stop_sha(&self, session_id: &str, path: &Path) -> Option<String>
```

This loads state once and looks up the path. The validator executor will batch one `load()` per Stop and consult the resulting `HashMap` directly, so prefer exposing a `load_last_stop_shas(&self, session_id) -> HashMap<PathBuf, String>` helper that returns the map in one shot. The implementer should pick whichever shape avoids re-loading inside a per-file loop.

### 3. Wire baseline write into allowed-Stop cleanup (strategy.rs)

In `ClaudeCodeHookStrategy::maybe_cleanup_turn_state` at `avp-common/src/strategy/claude/strategy.rs:412-437`, *before* the existing `clear()`/`clear_diffs()`/`clear_pre_content()` calls, load the current `TurnState` and call `record_stop_baseline(session_id, &state.changed)`. Log failures at `tracing::warn!` matching the existing pattern. The subsequent `clear()` then wipes `pending` and `changed` but `record_stop_baseline` has already written `last_stop_shas` into the saved YAML — the `clear()` call must be checked: today it deletes the YAML file entirely, which would also wipe `last_stop_shas`.

**Read the existing `clear()` implementation in `state.rs:185` first.** If it deletes the file, change the cleanup ordering: call `record_stop_baseline` first, but make it write the baseline into a state that is *already* otherwise empty. Concretely: load → empty `pending` and `changed` in-memory → fill `last_stop_shas` → save. Drop the call to `clear()` in this path (the diffs / pre-content sidecar cleanups still run).

This keeps the on-disk YAML a single source of truth for the baseline. Do not introduce a separate file.

### 4. Unify the two Stop-time lookups (validator_executor.rs)

`load_changed_files_for_stop` (line 83) and `load_diffs_from_sidecar` (line 124) both answer "what changed in this turn?" from overlapping sources but with different return shapes (paths vs. `FileDiff`). Replace them with one private function:

```rust
/// Effective set of files changed since the last allowed Stop.
///
/// Reconciles `turn_state.changed` and the sidecar diff directory into
/// a single candidate set, then drops any file whose current on-disk
/// SHA-256 matches the SHA recorded at the last allowed Stop. The
/// surviving entries carry the sidecar diff text (or none, when the
/// file is in `state.changed` but has no sidecar diff yet — same
/// behaviour as today's path-only fallback).
fn effective_changed_for_stop(&self, input: &I) -> Vec<crate::turn::FileDiff>
```

Then:

- The path-list call site (`load_changed_files_for_stop` consumers) becomes `effective_changed_for_stop(input).into_iter().map(|d| d.path.display().to_string()).collect()`. Return `None` when empty to preserve the existing `Option<Vec<String>>` contract.
- `prepare_diffs` (line 158) drops `load_diffs_from_sidecar` and uses `effective_changed_for_stop` directly when `chain_diffs.is_none() && hook_type == HookType::Stop`.

The hashing happens once per Stop inside `effective_changed_for_stop`. Files missing on disk are kept as candidates (so deletions remain visible). Files whose current SHA equals their `last_stop_shas` entry are dropped silently with a `tracing::debug!` count of how many were skipped.

Delete the two original methods. They are private to the impl block — no external callers.

### 5. SessionStart already covers `last_stop_shas`

`SessionStartCleanup` at `file_tracker.rs:371-393` calls `turn_state.clear(session_id)`. Since `last_stop_shas` lives inside the same YAML, the existing call wipes it as a side effect. No change needed there.

## Acceptance Criteria

- [ ] `TurnState` has a `last_stop_shas: HashMap<PathBuf, String>` field with `#[serde(default)]`; existing YAML without the field still deserializes.
- [ ] On every allowed Stop, `last_stop_shas` is replaced (not merged) with the SHA-256 of every file in `state.changed` that exists on disk.
- [ ] On a Stop validator run, a candidate file whose current on-disk SHA-256 equals its `last_stop_shas` entry is excluded from both the matched-files list and the diff list passed to validators.
- [ ] `load_changed_files_for_stop` and `load_diffs_from_sidecar` are replaced by a single `effective_changed_for_stop` function; the two old method names no longer exist.
- [ ] `effective_changed_for_stop` is called at most once per `prepare_diffs` invocation (no duplicate hashing).
- [ ] On SessionStart, `last_stop_shas` is wiped along with the rest of the state.
- [ ] Files in `state.changed` that no longer exist on disk are *not* skipped (deletions still flow through to validators).
- [ ] Blocked Stop iterations leave `last_stop_shas` untouched (`maybe_cleanup_turn_state` early-returns on block, as today).

## Tests

Write new tests in `avp-common/src/turn/state.rs` (unit, alongside the existing `mod tests`) and in `avp-common/src/chain/links/validator_executor.rs` (unit-style, using the existing test harness pattern there).

- [ ] **state.rs unit:** `record_stop_baseline` writes a SHA per existing path, omits paths that don't exist, and overwrites any prior `last_stop_shas` entries (replace-not-merge).
- [ ] **state.rs unit:** TurnState YAML round-trips with and without `last_stop_shas` populated; old YAML lacking the field still loads.
- [ ] **state.rs unit:** `clear` (or whatever the cleanup path becomes) wipes `last_stop_shas` on SessionStart but the allowed-Stop path *retains* it.
- [ ] **validator_executor.rs unit:** Given a session with two changed files, a baseline whose SHA matches one of them, and current on-disk content matching that SHA → `effective_changed_for_stop` returns only the other file.
- [ ] **validator_executor.rs unit:** Empty baseline → all candidates flow through (preserves first-turn behaviour).
- [ ] **validator_executor.rs unit:** A path in `state.changed` whose file has been deleted is still returned (deletions are visible).
- [ ] **validator_executor.rs unit:** A path whose sidecar `.diff` exists but whose current SHA matches the baseline is excluded from both the diff list and the matched-files list.
- [ ] **strategy.rs unit / integration:** allowed Stop with two changed files records two SHAs into `last_stop_shas` before the rest of the state is cleared; blocked Stop records nothing.
- [ ] **Regression:** add a test reproducing the kanban task `01KQ8CXYMBGN1VTV4S89FGQYCA` scenario (`turn_state.changed` empty, sidecars present) — the SHA filter must not regress that fallback path.
- [ ] `cargo nextest run -p avp-common` passes.
- [ ] `cargo clippy -p avp-common --all-targets -- -D warnings` is clean.

## Workflow

- Use `/tdd` — write the failing tests first (state.rs round-trip, baseline write, executor SHA filter), then implement the changes to make them pass.
- Read `state.rs:185` (`clear`) and confirm whether it deletes the YAML file or empties it; that determines the cleanup ordering in step 3 above.
- Do not introduce a new on-disk file or directory. The baseline lives inside the existing `turn_state/<session_id>.yaml`.
- Do not touch `turn_diffs/` or `turn_pre/` layout. Sidecars stay as-is.
- Do not change validator code, rule loaders, or anything outside the three files listed in **What**.