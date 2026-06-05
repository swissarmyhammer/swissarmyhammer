---
assignees:
- claude-code
depends_on:
- 01KTBN925WPAWDYXS12W5HETEH
- 01KTBNHSR4EVTVJ35MGGD510R2
- 01KTBQR87DKQF750JTJ3G52FZR
position_column: todo
position_ordinal: '8980'
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

## Acceptance Criteria
- [ ] `scope_review(scope) -> WorkList` exists, no agent/LLM dependency; accepts exactly one of `file`/`glob`/`working`/`sha` (zero/multiple → error).
- [ ] `WorkList.change_purpose` is populated from commit/task context.
- [ ] Each distinct `(file, probe)` runs once and is shared across validators that declared it (no per-validator re-run).
- [ ] Each `FileChange` carries the structured semantic diff, a bounded `source_slice` (header + changed entities + window, NOT whole file), changed symbols, and the validator's `probe_results`.
- [ ] Files grouped under matching validators via `match_rules`; reuses git changes/diff, `match_rules`, `run_probes` — no re-implemented git/glob/probe logic.

## Tests
- [ ] Integration test (real temp git repo + code_context index): change a `.rs` file adding a duplicate function; `working` scope → the file appears under matching validators with a non-empty semantic diff, a `source_slice` that includes the changed function + imports but NOT unrelated distant code, and the duplication validator's `probe_results` containing the `duplicates` hit.
- [ ] Probe-dedupe test: two validators declaring the same probe on the same file → the underlying probe runs once (assert via a counting/fake code_context or a shared-result identity check).
- [ ] `change_purpose` is set from the commit message under `sha` scope; `glob` returns matched files as whole-content work with no diff; an unmatched `.lock` yields no validator work.
- [ ] `cargo test -p swissarmyhammer-validators review::scope` green.

## Workflow
- Use `/tdd` — assert the `WorkList` shape (change_purpose, bounded source_slice, deduped probe_results) first. Reuse git ops, `match_rules`, `run_probes`, and the semantic differ's entity boundaries for the slice; if the git tool is not library-callable from the engine, factor the shared git-ops call site (note it in the design doc/board).