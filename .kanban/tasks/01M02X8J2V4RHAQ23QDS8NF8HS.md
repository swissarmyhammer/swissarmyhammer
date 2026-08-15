---
assignees:
- claude-code
position_column: todo
position_ordinal: ffe380
title: A duplication finding must be fixed on the changed side, not the counterpart
---
Under a diff op the engine enforces where a finding LANDS, but nothing states where its REMEDY may land. For duplication that gap is load-bearing, because a duplication finding is inherently a PAIR: the change on one side, a pre-existing block on the other. A finding correctly landing on a changed line still carries a remedy that points at the unchanged counterpart, and the fix edits a file the change never touched.

## What

Two edits, one concern: state the direction of the fix, in the two places an agent reads it.

**1. `builtin/validators/duplication/rules/duplication.md:71`** currently ends:

> The fix is always the same: extract a shared function and parameterize the difference.

That is direction-neutral. Given a pair, "extract a shared function" reads equally as "edit the pre-existing block". Rewrite it so the changed side is the subject:

- The changed code is what is under review. The remedy lands there.
- Where the counterpart already exists, the fix is to CALL it from the new code, not to rewrite it.
- Where extraction is genuinely needed, extract from the changed code. Touching the counterpart is a separate change and belongs to a separate task.

**2. `crates/swissarmyhammer-validators/src/review/probes.rs:743`** — the index-side `duplicates` rows built inside `run_duplicates` (line 720) carry no `detail`, so the row names a path and a similarity with no statement of which side the change touched. The `ProbeResult.target` IS the changed file and `dup.file_path` IS the counterpart, so the direction is already known and simply not written down.

Give that row a `detail` naming the direction, the way the sibling paths already do — `changed_set_duplicates` (line 769, row at 796) writes `"changed-set duplicate of {} in {}"`, and `clone_sibling_row` (line 970) writes which side the change edited and which is unchanged.

## Acceptance Criteria

- [ ] `duplication.md` states that the remedy lands on the changed code, and that calling an existing counterpart is preferred over rewriting it
- [ ] An index-side `duplicates` row carries a `detail` that names which side the change touched, so the row alone tells a reader which file to edit
- [ ] The wording distinguishes the two cases — counterpart exists (call it) versus extraction needed (extract from the changed code)
- [ ] `cargo nextest run -p swissarmyhammer-validators` passes; `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean

## Tests

- [ ] In `crates/swissarmyhammer-validators/src/review/probes.rs`, extend the existing `duplicates_returns_the_index_hit_for_a_duplicated_function` (line 1270) to assert the row's `detail` names the changed target and the counterpart, not just the path and similarity
- [ ] Add a rendering assertion beside the existing row-format tests near line 1156, pinning the full rendered row text so the direction wording cannot be dropped silently
- [ ] Run `cargo nextest run -p swissarmyhammer-validators -E 'test(duplicates)'` — the new assertions fail against the current `detail: None` and pass after

## Workflow

- Use `/tdd` — write the failing assertions first, then implement.

## Why the existing guard does not cover this

`^apb04az` made a diff op review only added and modified lines, and `scope::line_is_reviewed` refutes a finding that lands elsewhere. That guard is about the finding's LOCATION. It cannot see a remedy that names another file in its prose, which is why duplication still produces edits outside the diff.

#tool-validators