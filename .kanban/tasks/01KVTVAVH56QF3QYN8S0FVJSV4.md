---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvv7266d0tj9jtdwz90q5n8k
  text: |-
    Picked up. Studied cascade groundwork from ^tajpmp0 in edit/mod.rs. Two seams currently return McpError on ambiguity: (1) resolve_pair's (Some anchor, Some literal) arm; (2) resolve_via_ladder's MatchOutcome::Ambiguous{candidates: Vec<Span>} arm. Span carries {range, start_line, end_line, text}.

    Design: resolve_pair returns Result<PairOutcome, McpError> where PairOutcome = Resolved(Resolution) | Ambiguous{candidates}. apply_all_pairs bubbles Ambiguous up WITHOUT writing (file unchanged, atomicity preserved — nothing committed). execute_edit converts Ambiguous into a SUCCESSFUL CallToolResult (CallToolResult::success) listing candidate line numbers + current text + surrounding context. Add `occurrence` (1-based) param to EDIT_FILE_PARAMS; when supplied and resolves to exactly one candidate, splice that one. replace_all=true stays GlobalLiteral (no ambiguity). Following /tdd: failing tests first.
  timestamp: 2026-06-23T21:44:27.597666+00:00
- actor: claude-code
  id: 01kvv7hq6nbmk83k55c2ryf9xx
  text: |-
    Implementation landed (TDD red->green). Changes in edit/mod.rs:

    NEW TYPES: Candidate{range,line,text,context}; PairOutcome{Resolved(Resolution)|Ambiguous{find,candidates}}; ApplyOutcome{Applied(String)|Ambiguous{find,candidates}}.

    CASCADE CHANGES (consuming the two seams from ^tajpmp0):
    - resolve_pair now returns Result<PairOutcome,McpError>. The (Some anchor, Some literal) arm builds two candidates (anchor whole-line + literal span) and calls disambiguate(). resolve_via_ladder maps MatchOutcome::Ambiguous{candidates: Vec<Span>} into Candidates via disambiguate() instead of erroring. NoMatch still errors. replace_all -> GlobalLiteral unchanged (no prompt).
    - disambiguate(pair, candidates): if pair.occurrence (1-based) selects exactly one candidate -> Resolved(Splice{candidate.range, pair.replace}); else (no hint OR out-of-range) -> Ambiguous (never mis-applies).
    - apply_all_pairs returns Result<ApplyOutcome,..>; an Ambiguous pair short-circuits the batch BEFORE applying later pairs -> working copy discarded, file byte-identical (atomicity preserved).
    - execute_edit: ApplyOutcome::Ambiguous -> CallToolResult::success(render_ambiguity_prompt) (is_error=Some(false)), file unchanged; the commit_content path only runs for Applied.

    PARAMS: added `occurrence` (ParamType::Integer, 1-based) to EDIT_FILE_PARAMS; EditPair gained occurrence: Option<usize>; read_occurrence() (canonical only, rejects 0/negative/non-int -> None); threaded through pair_finds_replaces + both normalize_edit_args call sites (top-level + per-edit entry).

    HELPERS: line_number_at, render_context (N: gutter, radius=2), candidate_for, render_ambiguity_prompt.

    description.md: added "Ambiguity and occurrence" section.

    6 new tests (RED confirmed 5 failed pre-impl; replace_all_true passed pre-impl as expected since that path is non-ambiguous): ambiguity_returns_candidates_not_error_and_file_unchanged, occurrence_selects_nth/_one_selects_first, occurrence_out_of_range_returns_candidates_unchanged, anchor_and_literal_both_present_surfaces_candidates, replace_all_true_has_no_ambiguity_prompt.

    Ambiguity trigger in tests: find with surrounding whitespace ("  foo()  ") that content lines lack -> NOT a literal substring (content.find None), but normalize_line (outer-trim only) matches 2 lines via line-block rung -> MatchOutcome::Ambiguous.

    VERIFICATION (all green):
    - cargo nextest run -p swissarmyhammer-tools edit:: files:: => 226 passed (was 220; +6).
    - cargo nextest run -p swissarmyhammer-tools => 1359 passed, 0 failed (was 1353; +6).
    - cargo test -p swissarmyhammer-tools --doc => 8 passed.
    - cargo clippy -p swissarmyhammer-tools -- -D warnings => clean (exit 0).
    - cargo fmt --check => exit 0.

    No prior edit test changed its expectation — no existing test asserted an ambiguity error string in this file; the cascade's prior tests use unique/not-found cases which still behave identically.
  timestamp: 2026-06-23T21:52:56.533397+00:00
- actor: claude-code
  id: 01kvv7ppc2af6m8303at4qnwbv
  text: |-
    really-done: verification command green. Ran double-check adversarial gate -> PASS (no logic defect found). It probed occurrence=0/out-of-range/off-by-one (idx-1 cannot underflow since read_occurrence filters n>=1), multibyte slicing (byte offsets land on char boundaries), context near file boundaries (saturating_sub/.max(1)/.min(total) guards), empty-candidate panic (Ambiguous always >=2; anchor+literal always 2 — no panic), replace_all bypass, and atomicity (no commit on ambiguity).

    double-check raised ONE advisory: requirement #4 (ambiguous later pair in a batch must not partially write) had no DIRECT test — only the not-found-error rollback case. Logic was already correct (apply_all_pairs returns ApplyOutcome::Ambiguous before any commit), but I added a test to close the gap: ambiguous_later_pair_does_not_partially_write_earlier_pair — edits [{one->ONE (would apply)}, {"  two  " (ambiguous)}] asserts result is successful listing AND file byte-identical. Passes.

    Re-verified after adding the test:
    - cargo nextest run -p swissarmyhammer-tools edit:: files:: => 227 passed (was 226; +1).
    - cargo clippy -p swissarmyhammer-tools -- -D warnings => clean (exit 0).
    - cargo fmt --check => exit 0.

    double-check's other two notes were cosmetic-only (candidate ordering for anchor-vs-literal occurrence; in-batch candidate line numbers reflect working copy not disk — accurate, never persisted) — no action. Moving to review.
  timestamp: 2026-06-23T21:55:39.522581+00:00
depends_on:
- 01KVTVACR1W8HFKFR8DTAJPMP0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd880
project: file-edit-tools
title: edit files — ambiguity returns candidates (not an error) + occurrence hint
---
## What
When a `find` has multiple confident matches and `replace_all` is false, `edit files` must return the candidate spans so the model disambiguates in one follow-up — instead of failing. Implemented in `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs` on top of the cascade (consumes `MatchOutcome::Ambiguous` from `swissarmyhammer-edit-match`, and the "resolving anchor AND literal match both present" case from the cascade task).

- On ambiguity, return a structured result listing each candidate: line number, current text, and a few lines of surrounding context. This is a *successful* tool result describing the choice, not an `McpError`.
- Add an `occurrence` param (1-based index) and/or a line-hint to `EDIT_FILE_PARAMS` so the model can point precisely on the retry; when supplied and it resolves to exactly one candidate, apply it.
- `replace_all: true` continues to replace every match with no ambiguity.

## Acceptance Criteria
- [ ] Two+ confident matches with `replace_all` false return candidates (line numbers + current text + context), not an error, and the file is unchanged.
- [ ] Supplying `occurrence: N` selects the Nth candidate and applies the edit.
- [ ] A resolving anchor that also has a literal match surfaces both as candidates rather than guessing.
- [ ] `replace_all: true` replaces all matches with no candidate prompt.

## Tests
- [ ] Unit tests: ambiguous match returns candidate list + file untouched; `occurrence` disambiguates and applies; anchor-vs-literal both surfaced.
- [ ] `cargo test -p swissarmyhammer-tools edit::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.