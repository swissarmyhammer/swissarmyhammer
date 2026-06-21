---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvjvcd482w4fvfmsnhty3v8z
  text: Picked up via /finish. Work happens in the `swissarmyhammer-search` worktree (branch `search`, merged HEAD 51ec67fa6) — that's the checkout with the post-merge code. Sibling `swissarmyhammer/` is on `main`; do NOT edit there. Implementing the tracking-layer path normalization fix + the two real-path tests from the acceptance list.
  timestamp: 2026-06-20T15:46:24.008378+00:00
- actor: wballard
  id: 01kvjvf37zv7k1kvgg5zc9ew4y
  text: 'Research done. Confirmed the working-scope path is symmetric and relative on both ends as the description noted (resolve_working builds git-relative `files`, uses them for both subtract_unchanged lookup and the work-list; record_baseline_if_working records from that same work-list). The path-form fix is hardening, not a hot-path bug as the code reads. Now writing the two RED tests first (TDD): (1) production round-trip in scope.rs via record_baseline_if_working using FleetTally::new(1,0); (2) path-form defense in tracking.rs (absolute record -> relative subtract and vice versa). Will report whether the round-trip already passes pre-fix.'
  timestamp: 2026-06-20T15:47:52.191449+00:00
- actor: wballard
  id: 01kvjvhvc1qx9197w2bhc70p48
  text: |-
    KEY DATA POINT: the production round-trip test (working_round_trip_subtracts_after_real_baseline_record) PASSES before any normalization change. Real first pass -> real record_baseline_if_working(FleetTally::new(1,0)) -> second pass over unedited content subtracts the file, GREEN with zero code changes. This confirms the working-scope hot path is symmetric/relative on both ends; the re-review storm is NOT a record<->lookup key mismatch in the working path. The path-form defense IS hardening for the case some future caller hands tracking an absolute path.

    The two new path-form defense tests (subtract_matches_a_relative_spelling_against_an_absolute_recording + the absolute<->relative mirror) FAIL RED for the right reason before the fix: an absolute-spelled record and a relative-spelled subtract resolve to different keys, so subtract returns the file instead of dropping it. Implementing rel_key normalization now.
  timestamp: 2026-06-20T15:49:22.433240+00:00
- actor: wballard
  id: 01kvjvyf9jft7j2hfh7gnxw0g1
  text: |-
    Implementation landed. Added rel_key(repo_path, path) -> Option<String> + clean_relative helper in review/tracking.rs as the single normalization choke point. Routed entry_path, read_entry, upsert_entry, record_reviewed, and subtract_unchanged through it: write side (record) builds the TrackingEntry with the normalized key as its `path` so the stored context_hash is keyed on the canonical relative path (contract H(relative_path ‖ content ‖ rules_hash) preserved); read side normalizes the candidate before computing context_hash and reading the entry, so both ends always agree. Confinement: clean_relative resolves `.`/`..` lexically and rejects any `..` that climbs above the repo root or any residual RootDir/Prefix (mirrors drive.rs::confine_under_repo); upsert_entry has a belt-and-suspenders escape->error guard. Survivors returned from subtract_unchanged keep the caller's original spelling.

    Tests RED->GREEN:
    - Two path-form defense tests in tracking.rs (abs-record/rel-subtract and the rel-record/abs-subtract mirror): FAILED before the helper (subtract returned the file), PASS after.
    - Production round-trip in scope.rs (working_round_trip_subtracts_after_real_baseline_record): uses real scope_review + real record_baseline_if_working(FleetTally::new(1,0)), NOT a hand-seeded entry. It PASSED before the normalization too — confirming the working-scope hot path was already symmetric; the fix is hardening against a future caller emitting absolute paths.

    Verification (actual output):
    - cargo test -p swissarmyhammer-validators --lib review::  => 147 passed; 0 failed
    - cargo test -p swissarmyhammer-validators (whole crate)   => 315 passed; 0 failed + 2 doc-tests passed
    - cargo clippy -p swissarmyhammer-validators --all-targets => clean (fixed one question_mark warning with parts.pop()?)
    - Adversarial double-check agent: VERDICT PASS (could not fork a key, escape confinement, crash, or find a hollow test).

    Branch verified `search` before and after edits. Ready for /review.
  timestamp: 2026-06-20T15:56:16.050532+00:00
- actor: wballard
  id: 01kvjwng2pgbyg7vgfrydftmwc
  text: |-
    Addressed the two review-finding nits in review/tracking.rs (both doc-comment requests on public fns).

    Finding line numbers (33, 196) were against the pre-fix revision. In the current `search`-branch file both functions already carry doc comments added alongside the rel_key fix:
    - `read_entry`: already documents purpose, the None-on-missing/unreadable contract, AND the rel_key normalization ("normalized through [`rel_key`] so the read side resolves to the same entry file the write side produced"). Accurate and complete — left as-is.
    - `upsert_entry`: had a doc comment but it did not mention the rel_key normalization. Strengthened it to state the entry's `path` is normalized through [`rel_key`] to the canonical repo-relative key (so absolute/relative spellings collapse to one `.validators/.hashes/<key>.yaml`), and corrected the # Errors section to list AvpError::Context (path escapes repo) in addition to Io/Json.

    Documentation-only change, no behavior change — no new test (the 315 existing tests already cover these fns).

    Verification (actual output, exit 0):
    - cargo build -p swissarmyhammer-validators => Finished, clean
    - cargo clippy -p swissarmyhammer-validators --all-targets => Finished, zero warnings

    Branch confirmed `search`. Both checklist nits flipped to [x]. Moving back to review.
  timestamp: 2026-06-20T16:08:50.518567+00:00
- actor: claude-code
  id: 01kvjx5z6nmjakhgrt2ye6vbqq
  text: |-
    Triaged the re-review's findings against the actual code (verified, not trusted):

    - BLOCKER (subtract_unchanged reads before rel_key) — FALSE POSITIVE / misattributed. `subtract_unchanged` reads no file from disk; the candidate content is pre-read by `resolve_working` and passed in. The only read in the lookup path is `read_entry`, which goes through `entry_path` → `rel_key` confinement FIRST and returns None (no read) on escape.
    - Real but benign: `record_reviewed` does `repo_path.join(path)` + read BEFORE `rel_key`. For an absolute out-of-repo path this transiently reads then discards (rel_key→None skips the upsert, nothing stored). Real callers pass relative work-list paths, so not exploitable. Closing it anyway by normalizing key-first, then reading the confined `repo_path.join(&key)`.
    - Doc-comment warnings on read_entry/upsert_entry — FALSE POSITIVE: both carry full doc comments (lines 314, 324) added with the fix. No change.
    - `&[String]` / `&[(String,String)]` signature nits — declined as scope-creep micro-opts; out of scope for this task.

    Doing one tight hardening pass on record_reviewed + a confinement test, then a final re-review. This non-deterministic per-pass nit churn is itself the storm this task is about — will not loop indefinitely.
  timestamp: 2026-06-20T16:17:50.293821+00:00
- actor: wballard
  id: 01kvjxdb3cshydec4r4ghg08te
  text: |-
    Tight hardening pass on record_reviewed landed (the one item from the prior triage comment).

    Change (tracking.rs, scoped exactly to record_reviewed):
    - Reordered so rel_key(repo_path, path) is computed/confined FIRST (continue on None), THEN content is read from the confined location repo_path.join(&key) — previously it read repo_path.join(path) first, which for an absolute out-of-repo path is a transient read of a foreign file before rel_key discards it. Now an out-of-repo absolute path can never cause any read outside the repo. Normal relative-path behavior unchanged.
    - Updated record_reviewed's doc comment to state the path is normalized/confined before any read.

    Test (one added): record_reviewed_confines_an_out_of_repo_absolute_path — records with [relative in-repo path + absolute out-of-repo path] in one call; asserts the relative path records (read_entry is_some), the call does not error/panic (.unwrap), and no .validators/.hashes/ entry contains the out-of-repo content (recursive walk_files helper). Load-bearing: against the pre-reorder body it fails (raw absolute join reads the foreign file, then entry_path->None->Err->unwrap panics).

    Did NOT touch (per triage, declined): subtract_unchanged (verified false-positive — reads no files), read_entry/upsert_entry doc comments (already complete), &[String]/&[(String,String)] signature micro-opts (scope-creep).

    Verification (actual output, exit 0):
    - cargo test -p swissarmyhammer-validators --lib review::tracking => 24 passed; 0 failed (292 filtered out)
    - cargo clippy -p swissarmyhammer-validators --all-targets => Finished, zero warnings
    - Adversarial double-check agent: VERDICT PASS (confinement traced, normal paths preserved, test confirmed non-vacuous, no regressions).

    Branch confirmed `search`. Moving back to review.
  timestamp: 2026-06-20T16:21:51.852147+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffca80
title: Review-engine incremental hash-skip re-review storm (post-search-merge)
---
## Report (from monitoring agent)

A `/finish` review run showed a re-review storm: ~130 review fan-out agents at just 2 tasks (≈3× the prior run's 46), tokens ≈2.2× the prior run at the same point. The incremental `.validators/.hashes/` skip is being defeated — files get re-reviewed every round instead of subtracted.

Observed telemetry:
- 5 of 6 hash files frozen at `reviewed_at` 14:20:22; one (`ast.rs`) updated at 14:39:50.
- Storm: `lib.rs` ×3, `ast.rs` ×6 in back-to-back rounds.
- code_context's own ops are fine (0 errors); the breakage is scoped to the review engine's path/tracking handling.

Agent's hypothesis: the search-branch merge made the review engine emit inconsistent file paths (sometimes relative `src/ast.rs`, sometimes absolute `/Users/.../src/ast.rs`). The hash store (`crates/swissarmyhammer-validators/src/review/tracking.rs`) keys entries by relative path, so absolute-path reviews miss the lookup and never subtract.

## Verification so far (do NOT trust the hypothesis blindly)

Read the full flow — `tracking.rs`, `scope.rs::resolve_working`, `synthesize.rs::run_review`, `drive.rs::run_review_over_agent`, `review_op.rs`. As written, the working scope is **symmetric and relative on both ends**:
- `resolve_working` builds `files` from git status (repo-relative) and uses those same paths for BOTH `subtract_unchanged` (lookup) and the `work` list.
- `run_review` records the baseline from that SAME `work` list + SAME `repo_path` via `record_baseline_if_working` → relative keys.
- `run_review_over_agent` passes `repo_path` straight through unchanged.

So the "absolute vs relative emitted by the engine" mechanism is **not present** in the code as read. The on-disk `.validators/.hashes/*.yaml` keys are all relative.

Two alternative explanations to rule in/out:
1. **The storm may be expected** — frozen `reviewed_at` is also the signature of a file being *correctly* subtracted every round; `ast.rs ×6` may just be 6 real edits across 6 fix rounds. Need to confirm it's actually a regression, not normal fix-loop churn.
2. **`repo_path` instability between review invocations** — if successive review calls in one `/finish` run pass a different CWD/`repo_path`, `.validators/.hashes/` resolves to different dirs and entries written by one pass aren't found by the next. This IS a real path bug but a *different* one than reported.

## The fix: make the hash store self-protecting against path form (PRIMARY)

Regardless of which root cause is real, the tracking layer must NOT depend on every upstream caller handing it a consistent path form. It should **normalize at its own boundary** so relative and absolute spellings of the same file collapse to one key — the dedup protects itself:

- Normalize every path to a **canonical repo-relative key** inside `tracking.rs`, at the single choke points: `entry_path`, `read_entry`, `record_reviewed`/`upsert_entry`, and `subtract_unchanged`. Both the write side (record) and the read side (lookup) run the same normalization, so an absolute path written one pass and a relative path read the next (or vice versa) resolve to the identical `.validators/.hashes/<rel>.yaml`.
- Normalization rule: if the incoming path is absolute (or is under `repo_path`), `strip_prefix(repo_path)` to get the repo-relative key; if already relative, use as-is (after `.`/`..` cleanup). Both `repo_path` and the candidate should be canonicalized consistently so symlinked/`..` spellings don't fork keys. Do this in ONE helper (e.g. `fn rel_key(repo_path, path) -> String`) that every entry-path/read/record/subtract call routes through — single source of truth, no drift.
- Keep the stored `context_hash` keyed on the **normalized relative path** (matching the current doc contract `H(relative_path ‖ content ‖ rules_hash)`), so an absolute-path caller can never compute a different context hash for the same file.
- Guard against path-escape: a normalized key must stay within `.hashes/` (no `..` climbing out) — mirror the confinement check already in `drive.rs::read_text_file_under_repo`.

Net effect: even if some future caller (or a merge) regresses the engine back to emitting absolute paths, the skip still works because the store canonicalizes both ends to the same key.

## Acceptance / next steps

- [ ] Add the missing **production round-trip test**: first `review working` pass → real `record_baseline_if_working` (not a hand-seeded entry) → second pass over unedited content must subtract the file. (Existing `tracking_subtracts_*` tests seed entries manually and can't catch a record↔lookup key mismatch — see [[fixture-only-anti-pattern]], [[real-path-tests-not-mocks]].) Draft test was written in `review/scope.rs::tests` then reverted; re-add it.
- [ ] Add a **path-form defense test**: record a baseline using an ABSOLUTE path and assert a subsequent `subtract_unchanged` with the RELATIVE spelling (and vice versa) still subtracts — proves the store self-protects.
- [ ] Implement the single `rel_key` normalization helper in `tracking.rs` and route `entry_path`/`read_entry`/`record_reviewed`/`subtract_unchanged` through it.
- [ ] Run all of the above RED/GREEN on the merged `search` branch.
- [ ] If the round-trip already passes pre-fix: still land the normalization as hardening, then investigate `repo_path`/CWD stability across review calls in a real `/finish` run and/or confirm the storm is normal churn. Capture the actual log lines showing relative vs absolute paths the monitoring agent referenced.

## Key files
- `crates/swissarmyhammer-validators/src/review/tracking.rs` (hash store, relative-keyed — add `rel_key` normalization here)
- `crates/swissarmyhammer-validators/src/review/scope.rs` (`resolve_working`, subtract)
- `crates/swissarmyhammer-validators/src/review/synthesize.rs` (`run_review`, records baseline)
- `crates/swissarmyhammer-validators/src/review/drive.rs` (`run_review_over_agent`, has the in-repo confinement check to mirror)
- `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs` (`repo_path` resolution)

## Review Findings (2026-06-20 11:01)

### Nits
- [x] `crates/swissarmyhammer-validators/src/review/tracking.rs:33` — `pub fn read_entry` is a public function that lacks a documentation comment. Public functions should explain their purpose and behavior. Add a doc comment explaining the function, e.g., `/// Reads a stored tracking entry for the given path, returning None if not found.`.
- [x] `crates/swissarmyhammer-validators/src/review/tracking.rs:196` — `pub fn upsert_entry` is a public function that lacks a documentation comment. Public functions should document their purpose and behavior. Add a doc comment explaining the function, e.g., `/// Writes or updates a tracking entry, ensuring the .gitignore is configured.`.

## Review Findings (2026-06-20 11:09)

### Blockers
- [ ] `crates/swissarmyhammer-validators/src/review/tracking.rs:292` — Path traversal: file is read before path validation. If an absolute path is passed (e.g., `/etc/passwd`), `PathBuf::join()` returns the absolute path instead of joining with the repository base, allowing access to arbitrary files outside the intended repository. Move path validation before file read. Reorder to call `rel_key()` first, then read using the validated result: `let Some(key) = rel_key(repo_path, path) else { continue; }; let Ok(content) = std::fs::read_to_string(repo_path.join(&key))?`.

### Warnings
- [ ] `crates/swissarmyhammer-validators/src/review/tracking.rs:210` — Public function `upsert_entry` lacks a doc comment, leaving its purpose and error conditions unclear to callers. Add a doc comment explaining what the function does, its parameters, return value, and possible errors (e.g., filesystem failures, serialization errors).
- [ ] `crates/swissarmyhammer-validators/src/review/tracking.rs:230` — Parameter `files: &[String]` accepts concrete owned strings instead of references, forcing unnecessary allocations. Change the parameter type from `&[String]` to `&[impl AsRef<str>]` to allow callers to pass `&[&str]` without allocating.
- [ ] `crates/swissarmyhammer-validators/src/review/tracking.rs:245` — Parameter `candidates: &[(String, String)]` accepts concrete owned types instead of references. This forces callers to allocate owned Strings even when passing temporary values. Change the parameter to `&[(&str, &str)]` so callers can pass `vec![('path', 'content')]` without allocating.
- [ ] `crates/swissarmyhammer-validators/src/review/tracking.rs:295` — Public function `read_entry` lacks a doc comment. Add a doc comment describing the function's purpose, return value semantics (what None means), and behavior.

### Nits
- [ ] `crates/swissarmyhammer-validators/src/review/tracking.rs:450` — Public function `upsert_entry` lacks documentation explaining its purpose and behavior — callers cannot understand what it does or when to use it without reading the implementation. Add a doc comment explaining that this function writes a tracking entry to disk at the canonical path-mirrored location, creating parent directories and gitignore as needed.
- [ ] `crates/swissarmyhammer-validators/src/review/tracking.rs:500` — Public function `read_entry` lacks documentation — callers cannot understand what it retrieves or the meaning of `None` without reading the code. Add a doc comment explaining that this function reads a tracking entry from disk at the canonical path-mirrored location, returning `None` if no entry exists for that path.
