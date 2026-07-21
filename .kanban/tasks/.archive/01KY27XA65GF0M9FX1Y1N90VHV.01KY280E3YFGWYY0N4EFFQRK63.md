---
assignees:
- claude-code
position_column: todo
position_ordinal: ae80
title: 'review: LLM preflight stage — agent pass filters the scoped work-list before validators fan out'
---
## What

**Context.** Today the only in-engine scope filtering is deterministic: `.reviewignore` + `.gitignore` in `crates/swissarmyhammer-validators/src/review/ignore.rs` (hard-coded path rules). Judgment-based filtering — e.g. the /review skill's "blanket exception" that drops findings about restyling pre-existing tests (`builtin/skills/review/SKILL.md`, ~line 214) — lives only in skill prompt text, applied *after* validators have already burned a full fan-out reviewing those files. The ask: a **builtin LLM preflight** — an agent pass over the scoped files that filters the work-list down *before* the validator fleet runs, so out-of-scope files never cost validator turns.

**Design.** New engine stage between stage 1 (`scope_review`) and stage 2 (`batch_work_list`) of `run_review` in `crates/swissarmyhammer-validators/src/review/synthesize.rs`:

- [ ] New module `crates/swissarmyhammer-validators/src/review/preflight.rs` (declare + re-export in `crates/swissarmyhammer-validators/src/review/mod.rs`): `pub async fn preflight_work_list(work: WorkList, pool: &AgentPool, progress: Option<&ReviewProgressSender>) -> WorkList`. It submits ONE task to the shared `AgentPool` (`pool.submit`, same seam the fleet uses) whose prompt carries the `change_purpose` plus, per distinct file (`WorkList::distinct_files`), the path, `changed_symbols`, and the semantic-change kinds from `FileWork::semantic_diff` — cheap metadata, not full sources. The embedded prompt const (`PREFLIGHT_PROMPT`, styled after `OUTPUT_CONTRACT` in `fleet.rs`) states the builtin policy — drop files whose only change is restyling/refactoring *pre-existing test code*, generated/vendored files, and lockfile-style noise; keep everything with production or new-test changes — and demands a strict JSON array of `{"file": "...", "verdict": "keep"|"drop", "reason": "..."}`.
- [ ] Filtering is **conservative and fail-open**: only an explicit, parseable `"drop"` verdict removes a file; a file missing from the response, an unparseable response, or a failed/errored preflight task keeps the full work-list untouched (log a `tracing::warn!` and proceed). Every drop logs a `tracing::info!` with the file and the model's FULL reason — never truncated.
- [ ] Add `WorkList::retain_files(&self, keep: impl Fn(&str) -> bool) -> WorkList` (or equivalent) in `crates/swissarmyhammer-validators/src/review/scope.rs` that filters every `ValidatorWork::files` list and drops validators left with zero files, so the fleet plans zero pairs for dropped files (the `Planned` totals shrink accordingly — no phantom progress units).
- [ ] Wire the stage into `run_review` (`synthesize.rs`, between `scope_review` at ~line 353 and `batch_work_list` at ~line 357), before batching so batch budgets are computed over the filtered set.

Out of scope (follow-up cards if wanted): a `ReviewProgressEvent` variant announcing preflight drops to MCP clients; project-local override of the preflight policy; then relaxing the /review skill's hard-coded blanket exception to lean on the engine precheck.

## Acceptance Criteria

- [ ] A scoped work-list containing a file the preflight agent verdicts `"drop"` reaches `run_fleet` without that file: no `PairStarted` is emitted for it and the summed `Planned` totals exclude its pairs.
- [ ] A preflight response that is unparseable, missing files, or an outright pool task failure leaves the work-list byte-identical (fail-open) — the review runs exactly as it does today.
- [ ] A validator whose every file was dropped is removed from the work-list (fans out zero tasks) rather than fanning out with an empty file list.
- [ ] Each dropped file produces one `tracing::info!` naming the file and the full untruncated reason.

## Tests

- [ ] Unit tests in `crates/swissarmyhammer-validators/src/review/preflight.rs` `#[cfg(test)]` using the scripted-agent test support (`crates/swissarmyhammer-validators/src/review/test_support.rs`, `ScriptedReply`): (a) an explicit drop verdict removes the file from every `ValidatorWork`; (b) unparseable reply → work-list unchanged; (c) pool task error → work-list unchanged; (d) file absent from the verdict array → kept.
- [ ] Pipeline test alongside the existing scripted-agent pipeline tests in `crates/swissarmyhammer-validators/src/review/drive.rs` `#[cfg(test)]` (pattern: `review_working_drives_the_pipeline_over_a_scripted_agent`): script a preflight reply dropping one of two files and assert the fleet's submitted prompts and progress events cover only the kept file.
- [ ] `cargo test -p swissarmyhammer-validators review` — all green, each new unit test under 10s (scripted agent, no real model).

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.