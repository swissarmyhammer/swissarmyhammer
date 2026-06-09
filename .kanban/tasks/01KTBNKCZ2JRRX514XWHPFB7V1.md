---
assignees:
- claude-code
depends_on:
- 01KTBN925WPAWDYXS12W5HETEH
- 01KTBNHSR4EVTVJ35MGGD510R2
- 01KTBQR87DKQF750JTJ3G52FZR
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff380
project: local-review
title: 'Engine stage 1 — scope: resolve file/glob/working/sha to a per-validator work-list'
---
## What
First engine stage in `swissarmyhammer-validators::review`. Given a review scope, produce a `WorkList`: the review-level change purpose, plus per matched validator the files to review — each file carrying its structured diff, a bounded slice of source (NOT the whole file), and engine-run probe evidence. The validator is the shard; the **file is the grain**. This stage uses git + code_context only (no LLM) — deterministic and unit-testable.

- Input: a scope — exactly one of:
  - `working` — uncommitted changes vs HEAD (staged + unstaged + untracked); the default.
  - `sha` — changes in/since a commit or range (reuse the git tool's `get changes` range semantics).
  - `file` — a single file path: its changes if any, else whole-file content.
  - `glob` — all files matching a pattern.
- Reuse the git tool internals (`git/changes` + `git/diff`) for diff scopes; for `file`/`glob` resolve the file set directly. Do NOT shell out or reimplement diffing.
- **Change purpose**: gather the review-level intent once — commit message(s) for `sha`, the kanban task title+body when invoked task-mode, else a short auto summary. Attach to the `WorkList` (shared by every agent).
- For each file, call `match_rules(file_path)` to get matching validators; for each matched validator collect its declared `probes`.
- **Run probes deduped.** Run each distinct **`(file, probe)`** exactly once via `run_probes` (cache by `(file, probe)`), then attach the shared result to every validator on that file that declared it — do NOT re-run a probe per validator. For a large diff this is N+M probe calls, not N×M.
- **`FileChange` carries a BOUNDED slice, not the whole file** (token discipline): the file header (imports / module decl), each changed entity's full source (entity boundaries from the semantic diff), and a small window (~40 lines) around each hunk. For small files this is effectively the whole file.

Emit:
```
WorkList {
  change_purpose: String,
  validators: Vec<ValidatorWork {
    validator_name, severity, rules: Vec<Rule>, probes: Vec<ProbeName>,
    files: Vec<FileChange {
      path,
      semantic_diff,            // changed entities added/modified/deleted, before→after
      changed_symbols,
      source_slice,             // header + changed entities + window — NOT whole file
      probe_results,            // shared per-(file,probe) results for this validator's probes
    }>,
  }>,
}
```
A validator with no matched files is omitted.

## Implementation notes
- Lives in `crates/swissarmyhammer-validators/src/review/scope.rs`; re-exported from `review/mod.rs` as `scope_review`, `Scope`, `ScopeSpec`, `WorkList`, `ValidatorWork`, `FileWork`.
- The per-file work-item is named `FileWork` (the file is the grain) to avoid colliding with the existing `probes::FileChange` (a probe change-set) and `swissarmyhammer_sem::git_types::FileChange` (the differ input).
- **Factored shared git-ops call site** (the task's authorized path): the `git` MCP tool lives in `swissarmyhammer-tools`, which is NOT library-callable from the engine (it would invert the dependency direction). So the engine reuses the same underlying library crates the tool is built on — `swissarmyhammer-git` (`GitOperations`: working-tree status, range diffs, libgit2 blob/commit reads) and `swissarmyhammer-sem` (`compute_semantic_diff`) — directly, never shelling out, never reimplementing diffing. Added both as deps of the validators crate (no cycle; both are leaf crates).
- Validator matching reuses the same `matching_rulesets` code path as `match_rules`, via a caller-supplied `ValidatorLoader` loaded once (not reloaded per file).
- Probe dedupe is a single `run_probes` call over the whole change set with the union of every declared probe; results fan out to validators by a pure filter (N+M, never N×M).

## Acceptance Criteria
- [x] `scope_review(scope) -> WorkList` exists, no agent/LLM dependency; accepts exactly one of `file`/`glob`/`working`/`sha` (zero/multiple → error). (`ScopeSpec::resolve`)
- [x] `WorkList.change_purpose` is populated from commit/task context.
- [x] Each distinct `(file, probe)` runs once and is shared across validators that declared it (no per-validator re-run).
- [x] Each `FileChange` carries the structured semantic diff, a bounded `source_slice` (header + changed entities + window, NOT whole file), changed symbols, and the validator's `probe_results`.
- [x] Files grouped under matching validators via `match_rules`; reuses git changes/diff, `match_rules`, `run_probes` — no re-implemented git/glob/probe logic.

## Tests
- [x] Integration test (real temp git repo + code_context index): change a `.rs` file adding a duplicate function; `working` scope → the file appears under matching validators with a non-empty semantic diff, a `source_slice` that includes the changed function + imports but NOT unrelated distant code, and the duplication validator's `probe_results` containing the `duplicates` hit.
- [x] Probe-dedupe test: two validators declaring the same probe on the same file → the underlying probe runs once (shared-result identity check).
- [x] `change_purpose` is set from the commit message under `sha` scope; `glob` returns matched files as whole-content work with no diff; an unmatched `.lock` yields no validator work.
- [x] `cargo test -p swissarmyhammer-validators review::scope` green (9/9; full crate 167 + 2 doc-tests; clippy clean).

## Workflow
- Use `/tdd` — assert the `WorkList` shape (change_purpose, bounded source_slice, deduped probe_results) first. Reuse git ops, `match_rules`, `run_probes`, and the semantic differ's entity boundaries for the slice; if the git tool is not library-callable from the engine, factor the shared git-ops call site (note it in the design doc/board).

## Review Findings (2026-06-05 12:31)

Verified clean: builds, `cargo test -p swissarmyhammer-validators review::scope` 8/8, full crate 166 unit + 2 doc-tests, clippy `--all-targets` clean. Confirmed no dependency cycle — `swissarmyhammer-git` and `swissarmyhammer-sem` do not depend back on `swissarmyhammer-validators`; both are leaf library crates as the notes claim. Diff scopes reuse `GitOperations` (`get_status` / `get_changed_files_from_range` / `get_all_tracked_files`) and `compute_semantic_diff` directly — no shell-out, no reimplemented diffing. The `exactly-one-of` `ScopeSpec::resolve` gate, the bounded `source_slice` (header + entity bodies + ~40-line windows, distant code excluded — proven by the `distant_unrelated_marker` assertion), and the N+M probe dedupe (single `run_probes` over the probe union, shared byte-for-byte across validators — proven by `two_validators_share_one_probe_run_for_the_same_file`'s `a == b` identity check) all hold for the `duplicates` path.

### Warnings
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs` `probe_result_for_file` — symbol-targeted probe results (`callers`, `similar`) are silently dropped from every `FileWork`. The filter matches only `result.target == file || result.target == "<changed-set>"`, but `callers`/`similar` set `target` to a **symbol name** (e.g. `compute`), which equals neither — so a validator that declares `callers` or `similar` receives empty `probe_results`. Two of the three catalog probes never reach the work-list. The `duplicates`-only tests miss this because `duplicates`' per-file target IS a file path. The contract ("attach the shared result to every validator on that file that declared it") is only honored for one probe. Fix: resolve a symbol target back to its file via the changed entity that bears that name (the semantic diff already has `entity_name → file_path`), and add a test that declares `callers`/`similar` and asserts the result lands on the right file. **FIXED 2026-06-05:** `probe_result_for_file` now takes the file's `changed_symbols` (the per-file `entity_name → file_path` mapping the differ already produced) and matches `result.target` against it as a third resolution shape, so symbol-targeted `callers`/`similar` results attach to the file whose changed entity bears that name. New test `symbol_targeted_probes_attach_to_the_file_bearing_the_symbol` declares both `callers` and `similar` on the changed `.rs` file, seeds an inbound caller + a reuse candidate, and asserts each result lands on `src/lib.rs` (the file bearing `compute`). It failed RED before the fix ("callers result attaches to the file bearing `compute`") and is GREEN after.
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs` `probe_result_for_file` docstring — the doc claims "symbol targets (`callers` / `similar`) attach to the file whose changed entity bears that name," but the implementation does no symbol→file resolution; it drops them. The docstring describes behavior the code does not implement. Fix alongside the warning above (or correct the doc if the drop were intentional — it is not, given the catalog ships all three probes). **FIXED 2026-06-05:** the docstring now enumerates the three target shapes (file path, `<changed-set>`, symbol name) and describes the symbol→file resolution via `changed_symbols` that the code now actually performs.

### Nits
- [x] `crates/swissarmyhammer-validators/src/review/scope.rs` `scope_review` — `change_purpose` has no task-mode (title+body) path; `working`/`file`/`glob` all fall back to `auto_purpose`. The criterion is met for this stage (sha → commit message, else auto summary) and task-context plumbing belongs to a later wiring stage, but note the "kanban task title+body when invoked task-mode" half of the change-purpose spec is not yet reachable from this signature. **NOTED 2026-06-05:** added a "Change purpose" section to the `scope_review` docstring documenting that the task-mode (title+body) half is intentionally out of scope for the deterministic stage and is plumbed by a later wiring stage that wraps this call. No behavior change.