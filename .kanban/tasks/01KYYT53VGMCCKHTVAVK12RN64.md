---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyzkr6akxrzhs1yzqzsx8t2s
  text: |-
    ### implement — changed

    **Files touched**
    - `crates/swissarmyhammer-git/src/operations.rs` — new `LineBlame` enum (`Commit(sha)` / `Worktree` / `Untracked` / `Failed`, each with an 8-char `sha_label()`), new `GitOperations::blame_lines(path, content, newest)` (layers `blame_file` + `blame_buffer` so an uncommitted/dirty line reads as `Worktree`, bounded to `newest` for a historical `Sha` review), private `path_is_tracked` helper, 6 new tests.
    - `crates/swissarmyhammer-git/src/lib.rs` — export `LineBlame`.
    - `crates/swissarmyhammer-validators/src/review/scope.rs` — new `LineAnnotation` (sha + touched), `FileWork::line_annotations` (attached via `.with_line_annotations`, not a breaking ctor change), `ResolvedScope::blame_at` (the Sha-scope "to" commit anchor; `None` elsewhere), `compute_line_marks` (git2::Patch-based before/after diff — the ONLY source of the `+` mark, never blame), `compute_line_annotations` (one blame call per matched file, run concurrently via `tokio::task::spawn_blocking`, each opening its own `GitOperations` handle since `git2::Repository` is `Send` not `Sync`). 6 new tests + `resolve_oid` helper.
    - `crates/swissarmyhammer-validators/src/review/fleet.rs` — `render_numbered_source` + `LINE_FORMAT_LEGEND` replace the bare fenced block; `DEFAULT_BATCH_SIZE` raised 256 KiB → 384 KiB.
    - `crates/swissarmyhammer-validators/src/review/fleet/tests.rs` — updated the `DEFAULT_BATCH_SIZE` pin test.

    **Format** (exact spec from the task): `{line:>6} | {sha:8} {mark} | {text}`, with a legend above the block instructing the model to READ the number, not count.

    **Edge cases — all covered with real tests**
    - Uncommitted/dirty line → `worktree` (git's own "not committed yet" sentinel via `blame_buffer`).
    - Brand-new file, staged or untracked-but-code → `worktree` (tracked, no reachable commit yet) or `untrackd` (git doesn't track it at all) — distinguished via index/tree lookup, not a heuristic.
    - Deleted file → block stays empty, unchanged from prior behavior (verified, no new code path touches it).
    - Blame failure for any other reason → `????????`, `tracing::warn!`, review continues. `compute_line_annotations` has NO `Result` return — a blame failure is structurally incapable of aborting the review, not just tested not to.

    **A real bug caught by the tests**: my first pass trimmed content with `.trim_end()` *before* handing it to blame/diff, which byte-mismatched every file's last line against its committed blob (which ends in `\n`) — falsely marking almost every file's LAST line as `worktree`/touched. Fixed by diffing/blaming the RAW content and only trimming for the final render-facing line count. Caught by `working_scope_dirty_line_gets_worktree_sha` and `sha_scope_line_annotations_carry_correct_number_sha_and_mark` before this ever reached review runs.

    **batch_size decision**: kept the raw-`source_slice`-byte budget in `batch_work_list` (does NOT measure the rendered block) — avoids coupling the deterministic scope stage to the fleet stage's rendering. Raised `DEFAULT_BATCH_SIZE` 256 KiB → 384 KiB (1.5x) to comfortably clear the measured ~1.45x rendering overhead. Pinned by `default_batch_size_is_384_kib`.

    **Blame wall-clock**: `blame_overhead_on_a_representative_commit_is_small_and_concurrent` measures a full `scope_review` (semantic diff + probes + blame) over 8 files × 150 lines each with a dirty edit: **~10.6ms** end-to-end. Blame itself runs one call per file concurrently via `spawn_blocking`; negligible next to a real validator LLM turn (seconds–minutes).

    **Test counts**: swissarmyhammer-git 96 → 102 (+6). swissarmyhammer-validators 334 → 340 (+6, plus 1 modified). `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'`: 5031 passed, 0 failed, 2 skipped (including the real-model e2e `review_runs_over_acp_against_a_real_local_model`, which exercises this code on the real production path). `cargo clippy --workspace --all-targets -- -D warnings`: clean. `cargo fmt --all`: no diffs.

    next: /review
  timestamp: 2026-08-01T21:30:14.739171+00:00
position_column: doing
position_ordinal: '8280'
title: 'Review prime: number every line and show its blame commit'
---
# Problem 1: the model guesses the line number

The review prime gives the model the file source with no line numbers. `render_file_block` (`crates/swissarmyhammer-validators/src/review/fleet.rs:1411`) writes:

```rust
out.push_str("```\n");
out.push_str(file.source_slice().trim_end());
out.push_str("\n```\n\n");
```

But `Finding.line` (`crates/swissarmyhammer-validators/src/review/types.rs:27`) requires a value: "1-based line number the finding points at."

The model must count the lines to get this number. The model counts incorrectly.

## Evidence

Data from one `/finish #review` batch. "True" is the correct location, found by hand.

| Cited | True | Error |
|---|---|---|
| `scope.rs:32` | 63 | +31 |
| `scope.rs:108` | 122 | +14 |
| `scope.rs:114` | 128 | +14 |
| `scope.rs:589` | 818 | +229 |
| `review_op.rs:1016` | 1344 | +328 |
| `review_progress_notifications_test.rs:68` | 101 | +33 |
| `review_progress_notifications_test.rs:154` | 237 | +83 |
| `review_progress_stdio_test.rs:122` | 159 | +37 |
| `review_progress_stdio_test.rs:147` | 172 | +25 |
| `review_progress_stdio_test.rs:332` | 344 | +12 |
| `fleet/tests.rs:65` | 63 | -2 |

The error increases with the depth into the file. The error is approximately 330 lines at line 1344. The model usually reports a number that is too small.

This shape shows that the model estimates. A defect in diff mapping gives a constant error for each hunk, and you can calculate it exactly. An error that increases with depth does not.

# Problem 2: the model cannot see what is pre-existing

The review rules drop a finding that asks to refactor test code that already existed. To apply this rule, the reviewer must know if a line is new or old. The prime does not say. Each reviewer in the batch ran `git blame` by hand after the review, for each finding, to get this answer. The result controls if a finding is recorded or dropped, so this is not only slow — a wrong answer gives a wrong decision.

# Changes

## Format

Put the line number, the blame commit, and a change marker on each line of the block that `render_file_block` writes:

```
  61 | c691f8ec   | /// A review scope.
  62 | c691f8ec   |
  63 | a561c5b9 + | #[derive(Debug, Clone, Eq, Hash)]
  64 | c691f8ec   | pub enum Scope {
```

Exact layout for each line: `{line:>6} | {sha:8} {mark} | {text}`

- `line` — the 1-based line number, right-aligned in 6 columns.
- `sha` — the first 8 characters of the commit that last changed the line.
- `mark` — `+` if this change touched the line. One space if it did not.
- `text` — the source line, unchanged.

Write a legend above the block. Tell the model to read the number and not to count the lines.

## Get the marker from the diff, not from blame

The scope stage holds the before content, the after content, and the semantic diff. Calculate `mark` from that data. Do not use blame for the marker.

Use blame only to attribute a line to a commit.

## Edge cases

- **A line that is not committed** (a `review working` scope): `git blame` gives all zeros. Show `worktree` in the sha column.
- **A new file**: each line gets the commit of this change, or `worktree`.
- **A deleted file**: there is no current content. The block stays empty, as it is today.
- **A file that git does not track**: show `untrackd` in the sha column. Do not fail the review.
- **Blame fails** for any other reason: show `????????`, write a `tracing::warn!`, and continue. A blame failure must never stop a review.

# Two consequences to handle

## 1. `batch_size` no longer measures the prime

`batch_work_list` (`scope.rs:1304`) budgets on `file.source_slice.len()` — the raw source bytes. This change makes the rendered prime approximately 1.45 times larger than the raw source (about 16 bytes for each line, on source lines that average about 35 bytes). The budget will then not agree with the size of the text that the model receives.

Choose one, and record which:
- Measure the rendered block size in `batch_work_list`, or
- Keep the raw-source budget, document that the prime is larger, and raise `DEFAULT_BATCH_SIZE` (`fleet.rs:96`) so that a normal commit still fits in one batch.

Do not leave the disagreement undocumented.

## 2. Blame cost

Blame each file one time for each review run, not one time for each finding or for each validator. Use the existing `GitOperations`. Run the blame calls for different files at the same time. Report the added wall-clock time for a normal review in the task comments.

# Acceptance

- A production-path test. Give a file with known content and known history to the prime. Show that each line carries the correct number, the correct 8-character sha, and the correct marker.
- A test for the `review working` scope that shows an uncommitted line gets `worktree`.
- A test that shows a blame failure gives `????????` and the review still completes.
- A test that shows a finding keeps the correct line number from the prime to the report.
- A test that pins the `batch_size` decision above, whichever you choose.
- `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` passes.
- The prime must continue to give the complete file. Do not increase the count of read round-trips. #review