---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvwp7escxv4kk42mwc355xkq
  text: 'Picked up. BEFORE coverage: src/lib.rs lines 92.48% (266 lines, 20 missed). Uncovered: 145, 172, 181, 201, 243, 278, 279, 294, 313, 322, 351-354, 363-364, 383, 403, 407 — matches task description. Plan: add tests/edge_cases.rs exercising each uncovered behavior (Normalized Ambiguous, Anchor None/end<=start, CRLF alignment, trailing-empty trim, empty-find guards, levenshtein empty-a, byte_offsets overflow break, span_of empty-range, physical_lines no-final-newline). Following /tdd.'
  timestamp: 2026-06-24T11:28:43.308657+00:00
- actor: claude-code
  id: 01kvwpg6xrh4warj0cgqkjgvch
  text: |-
    DONE — moved to review. Added crates/swissarmyhammer-edit-match/tests/edge_cases.rs (14 new behavior tests, all assert MatchOutcome variant + spans):
    - Normalized-rung Ambiguous (duplicated de-indented block → 2 candidate spans) and trailing-blank-line trim (find ending \n\n → trim_trailing_empty drops phantom line, lines 363-364).
    - Anchor-rung None paths: non-unique first line (403), inverted anchors end_idx<=start_idx (407), empty first anchor line (383).
    - CRLF: single-line Exact line-aligned via \r\n with more following (278-279) and via trailing \r at EOF (280); multi-line Normalized over \r\n split (physical_lines 172); no-final-newline final push reached via Normalized rung (181).
    - Edge guards: empty find → Ambiguous over blank lines (try_exact 243 + try_line_block 313 return None, span_of empty-range fallback 201) and NoMatch on non-blank; find longer than content (322); repeated multi-line block to EOF (byte_offsets_of); similarity("","x")=0 (levenshtein empty-a 145).

    PRODUCTION SIMPLIFICATION (not a test contortion): byte_offsets_of had an unreachable overflow guard `if from > haystack.len() { break; }` plus `needle.len().max(1)`. Its sole caller try_exact returns early on empty find, so needle is always non-empty and `from = at + needle.len()` can never exceed len. Removed the dead break + redundant .max(1), added a doc comment explaining why. Behavior-preserving (confirmed by adversarial double-check tracing all callers).

    COVERAGE src/lib.rs: BEFORE 92.48% lines (20 missed) → AFTER 100.00% lines (264/264, 0 missed), functions 100%. Regions 99.56% (2 sub-line expect() panic arms that never fire — not lines, outside the line goal).

    Commands (repo root): `cargo nextest run -p swissarmyhammer-edit-match` → 31 passed, 0 skipped (was 17); `cargo test --doc -p swissarmyhammer-edit-match` → 1 passed; `cargo llvm-cov nextest -p swissarmyhammer-edit-match --summary-only` → lib.rs 100% lines; `cargo fmt` applied; `cargo clippy -p swissarmyhammer-edit-match --all-targets -- -D warnings` → clean. Adversarial double-check verdict: PASS. NOT committed (orchestrator commits).
  timestamp: 2026-06-24T11:33:30.168274+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffdf80
project: file-edit-tools
title: swissarmyhammer-edit-match — close test-coverage gaps (rung ambiguity, CRLF, edge guards)
---
## What
Test-only task: raise `crates/swissarmyhammer-edit-match/src/lib.rs` line coverage from 92.5% toward ~100% by exercising the currently-uncovered behaviors. No production code changes expected (if a gap turns out to be genuinely dead/unreachable, simplify rather than add a test, and note it). Coverage measured via `cargo llvm-cov nextest -p swissarmyhammer-edit-match`.

Uncovered lines (from `cargo llvm-cov report --show-missing-lines`): 145, 172, 181, 201, 243, 278, 279, 294, 313, 322, 351-354, 363-364, 383, 403, 407. Grouped by behavior:

- **Normalized-rung ambiguity (`finalize_block_matches`, lines 351-354)** — when `try_line_block` finds the SAME normalized line-block in two+ places, the result is `MatchOutcome::Ambiguous`. Add a test: content with a duplicated indented block, a de-indented `find` matching both → assert `Ambiguous` with the two candidate spans (no silent pick).
- **Anchor-rung non-unique hits (`try_anchor`, line 403)** — first/last anchor line not unique → returns `None` (descends). Add a test where the `find`'s first or last line appears on multiple content lines → assert the anchor rung does not match (falls through to fuzzy/NoMatch as appropriate). Also cover line 407 (`end_idx <= start_idx`): a `find` whose last-line match sits at/above its first-line match.
- **CRLF line handling (`physical_lines` 172, `is_line_aligned` 278-280)** — exercise CRLF content: an Exact single-line `find` that is line-aligned in a `\r\n` file (right boundary via `\r\n`), and a `\r` at end-of-content (line 280 branch). Assert correct line-aligned Exact match and correct spans on CRLF input.
- **Trailing-empty trim (`trim_trailing_empty` 363-364)** — a `find` with a trailing newline (so a normalized-empty trailing line) must match the intended block without the phantom empty line. Add a Normalized-rung test with a `find` ending in `\n`.
- **Edge guards** — empty `find` → `try_exact` 243 / `try_line_block` 313 return `None` (assert `find_match(content, "")` behavior is sensible/`NoMatch`); `find` longer than content (`try_line_block` 322); `byte_offsets_of` advance/overflow break (294) via a `find` whose repeats reach end-of-content; `span_of` empty-range fallback (201); `levenshtein` empty-`a` branch (145) via `similarity("", "x")`; `try_anchor` empty first/last (383) and `physical_lines` no-final-newline final push (181).

## Acceptance Criteria
- [ ] `cargo llvm-cov nextest -p swissarmyhammer-edit-match --summary-only` shows `swissarmyhammer-edit-match/src/lib.rs` at ≥99% lines (target 100%; if any line is genuinely unreachable, remove/simplify it instead and note which in a comment).
- [ ] New tests assert BEHAVIOR (correct `MatchOutcome` variant + spans), not just line execution — especially the Normalized and Anchor rung `Ambiguous`/`None` outcomes.
- [ ] CRLF inputs are exercised and produce correct line-aligned matches/spans.
- [ ] No production behavior change (existing tests stay green); any dead-code simplification is called out.

## Tests
- [ ] Add the cases above to `crates/swissarmyhammer-edit-match/tests/` (e.g. extend `rungs.rs` / add an `edge_cases.rs`) — NEVER plain `cargo test`; use `cargo nextest run -p swissarmyhammer-edit-match`.
- [ ] `cargo llvm-cov nextest -p swissarmyhammer-edit-match --summary-only` confirms the ≥99% line target.
- [ ] `cargo fmt` + `cargo clippy -p swissarmyhammer-edit-match --all-targets -- -D warnings` clean.

## Workflow
- Use `/tdd` — write the failing/uncovered-behavior tests first, confirm they exercise the target lines, then adjust.