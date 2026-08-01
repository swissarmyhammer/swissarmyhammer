---
assignees:
- claude-code
position_column: todo
position_ordinal: da80
title: batch_size is ignored at runtime; a 318 KB file cannot be reviewed by any route
---
# Problem

The `batch_size` modifier does nothing at runtime. A live call with `batch_size: 1` still reported the 262,144-byte default in its error message. The override does not reach the engine.

The source looks correct. Read it before you change it:

- `BATCH_SIZE_PARAM` is declared at `crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs:69`.
- It is in the parameter list of all three review ops: `review file` (`mod.rs:86`), `review working` (`mod.rs:123`), `review sha` (`mod.rs:136`).
- `mod.rs:365` reads it: `.with_batch_size(usize_arg(args, "batch_size"))`.
- `review_op.rs:525` uses it: `request.batch_size.map(FleetConfig::new).unwrap_or_default()`.
- `synthesize.rs:357` applies it: `batch_work_list(&work, fleet_config.batch_size())?`.
- `scope.rs:1304` does the packing and gives the error.

`BATCH_SIZE_PARAM` came in at commit `b4bac5136` on 2026-06-27, so an old binary is not a likely cause.

# First, find the true cause

Do not correct the code before you know why it fails. Possible causes:

1. The `review sha` and `review working` dispatch arms do not use the same request builder that `mod.rs:365` is part of. Find the function that holds line 365. Then confirm that each of the three ops goes through it.
2. Something removes unknown or extra arguments before the handler gets them.
3. `usize_arg` rejects the value. `usize_arg` (`crates/swissarmyhammer-tools/src/mcp/op_tool_helpers.rs:69`) treats a negative or fractional number as absent.
4. The running server is an old build.

Write the true cause in a task comment with the evidence. Then correct it.

# Problem 2: a file that no route can review

`crates/llama-agent/src/acp/server.rs` inlines 318,564 bytes. The default budget is 262,144 bytes. So `scope.rs:1312` gives a hard error and `review sha` stops. Because the override does not work, you cannot raise the budget to review this file. `review file` on that one file has the same limit.

The result: this file gets no review, and the run reports an error instead of a gap.

# Problem 3: the CLI has no way to do this

`sah tool review sha review --batch_size ...` fails with: "the `review` ops need a live agent; this tool was built without an agent factory". So the CLI is not an alternative route.

# Changes

1. Correct the cause you found, so that `batch_size` controls the budget for all three review ops.
2. Add the test that is missing today. `crates/swissarmyhammer-tools` has no test that contains the text `batch_size`. Everything below `FleetConfig::new` has tests. The hop that reads the JSON argument has none. This is why the defect was invisible.
3. Decide what a review does with a file that is larger than the budget, and record the decision. A hard error that stops the whole review is a poor result, because one large file then blocks the review of every other file. Better: review the other files, and report the large file as "not reviewed, too large" in the report. The user then sees a gap, not a failure.
4. Give the CLI a route to the review ops, or make the error name the correct alternative.

# Acceptance

- A production-path test for each of `review file`, `review working`, and `review sha`: pass `batch_size`, and show that the engine uses that value and not the default. This test must go through the registered tool, not a mock.
- A test that shows a wrong `batch_size` value (negative, fractional, zero) behaves as documented.
- A test that shows a file larger than the budget does not stop the review of the other files, and that the report names the file that was not reviewed.
- `crates/llama-agent/src/acp/server.rs` gets a review through a normal route.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'` passes. #review