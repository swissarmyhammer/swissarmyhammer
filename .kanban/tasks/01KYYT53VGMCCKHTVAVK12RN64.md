---
assignees:
- claude-code
position_column: todo
position_ordinal: d980
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