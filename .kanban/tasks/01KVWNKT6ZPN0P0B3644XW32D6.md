---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvwq504p0k3fy1d7b40060nn
  text: 'Picked up. Plan: TDD — write failing mtime-advances test in edit/mod.rs (seed old mtime via filetime::set_file_mtime, edit, assert new mtime strictly greater). Then remove original_modified/original_accessed capture + set_file_times in commit_content; delete metadata_preserved field from EditResult, result assembly, and mutation envelope. Keep set_permissions step (silent). Clean up unused filetime imports. cargo nextest + fmt + clippy.'
  timestamp: 2026-06-24T11:44:51.350315+00:00
- actor: claude-code
  id: 01kvwqhs44wav94ep1pe6fqh1y
  text: |-
    Implemented (TDD).

    RED: added test_edit_file_advances_modification_time in edit/mod.rs — seeds old mtime via filetime::set_file_mtime (FileTime::from_unix_time(1_000_000_000,0), no sleep), edits, asserts new mtime strictly greater. Watched it FAIL (new==old==1000000000, proving old mtime was restored).

    GREEN:
    - commit_content: removed original_modified/original_accessed capture and the set_file_times(...) call + its metadata_preserved match. Atomic rename's fresh mtime now stands. Updated doc + inline comments to say mtime is intentionally NOT preserved.
    - KEPT set_permissions(temp, original) before rename as SILENT behavior (comment added re: 0755 not downgrading to 0644).
    - Deleted metadata_preserved field from EditResult struct, from the EditResult{} construction, from the execute_edit mutation envelope JSON, and from both debug! tracing fields.
    - Removed now-unused `use filetime::{set_file_times, FileTime};` top-level import (filetime still used via local `use` in the new test; it's a declared dep).
    - Removed the metadata_preserved assertion in successful_edit_carries_tagged_content_and_mutated_paths.

    Verification (all fresh):
    - rg metadata_preserved crates/swissarmyhammer-tools = 0 hits (non-test AND test).
    - cargo nextest run -p swissarmyhammer-tools edit:: files:: = 233 passed, 0 failed (incl. new mtime test + test_edit_file_permissions_preservation green).
    - cargo nextest run -p swissarmyhammer-tools = 1367 passed, 0 failed.
    - cargo test -p swissarmyhammer-tools --doc = 8 passed, 0 failed.
    - cargo fmt applied; cargo clippy -p swissarmyhammer-tools -- -D warnings = clean.
    - Only edit/mod.rs changed (git diff --stat); write/ untouched.

    Adversarial double-check: PASS. mtime advances, perms preserved, zero metadata_preserved refs, no unused imports, write untouched. NOT committed.
  timestamp: 2026-06-24T11:51:50.148830+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe080
project: file-edit-tools
title: edit files — drop mtime preservation + the metadata_preserved return field (editing advances mtime)
---
## What
`edit files` preserves the original file **modification time** across a content-changing edit, so the file's mtime never advances — defeating every mtime-based staleness check downstream (`cargo`/`make` skip rebuilds, file watchers and rust-analyzer can miss the change, and it's a candidate cause of the inline-diagnostics fold-in returning empty after an edit). It also reports `metadata_preserved: true` in the result, spending return tokens on something the model does not need.

**Decision (from the user):** editing a file IS editing a file — the mtime must advance, and the result should NOT carry a `metadata_preserved` field. Remove both.

**Root cause** — `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs`, `commit_content`:
- ~1138-1139 capture `original_modified` / `original_accessed`.
- ~1190 `set_file_times(path, original_accessed, original_modified)` restores the old timestamps after the atomic temp-write + `fs::rename`, and reports `metadata_preserved`.

Confirmed in practice: an agent edited a `.rs` file to a compile error and `cargo check` returned exit 0 in 0.10s with NO recompile (mtime unchanged); only `touch` forced cargo to see it.

`write files` (`write/mod.rs` `write_file_atomic`) does NOT restore timestamps — it gets a fresh mtime — so this is edit-only. Do not change write.

## Approach (firm decisions)
1. **Drop modification-time preservation.** Remove the `original_modified`/`original_accessed` capture and the `set_file_times(...)` call in `commit_content` so a content edit gets the rename's natural current mtime. Do NOT add atime preservation back — just let it advance.
2. **Delete the `metadata_preserved` field entirely** (not redefine — delete, to save return tokens): remove it from `EditResult` (~line 907), from the success/structured result assembly, and from `mutation.metadata_preserved` in the envelope. Update/remove the structured-result test that asserts it (~line 3386).
3. **KEEP permission re-application as silent behavior.** The temp-write+rename gives the new file default permissions, so the existing step that re-applies the original mode (the `set_permissions` on the temp file before rename) must stay — otherwise editing an executable script (e.g. `0755`) silently downgrades it to `0644`. This is invisible (no return field) and prevents a real regression. Do NOT report it in the result.
4. This may be necessary-but-not-sufficient for the silent-diagnostics symptom (rust-analyzer is leader-gated/lazy — see the `diagnostics` project). Scope THIS task to the mtime + return-field cleanup; cross-reference diagnostics for the LSP-warmth angle.

## Acceptance Criteria
- [ ] After an `edit files` content edit, the file's modification time is **strictly greater** than its pre-edit mtime.
- [ ] The result (text + structured `mutation` envelope) contains **no** `metadata_preserved` field anywhere.
- [ ] File **permissions are still preserved** across an edit (e.g. a `0755` file stays `0755`) — silent, not reported.
- [ ] Encoding + line-ending preservation and atomicity unchanged; existing edit tests green.

## Tests
- [ ] New regression test in `edit/mod.rs`: create a file, seed a clearly-old mtime via `filetime::set_file_mtime` (a fixed past `FileTime`, no wall-clock sleep), edit it, assert the new mtime is strictly greater than the seeded old one.
- [ ] Keep `test_edit_file_permissions_preservation` (perms still preserved after the change).
- [ ] Remove/replace the `metadata_preserved` assertion at ~line 3386 and any other test asserting that field; grep to confirm no `metadata_preserved` references remain in the crate.
- [ ] `cargo nextest run -p swissarmyhammer-tools edit::` green (NEVER plain `cargo test`; doctests via `--doc`); `cargo fmt` + `cargo clippy -p swissarmyhammer-tools -- -D warnings` clean.

## Workflow
- Use `/tdd` — write the failing mtime-advances test first (fails today because the old mtime is restored), then implement.