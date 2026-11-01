---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztw1xr5b6gjfqbk3hthgf95
  text: |-
    ### The test passes at HEAD — measured 2026-08-12

    Found during the test step of ^wwb6hk7. `cargo nextest run --workspace` at commit 59bd9ae5c: 14136 passed, 0 failed. `mcp::tools::shell::tests::shell_description_states_blocking_and_no_tail` passes.

    **Cause: two events in sequence.** The merge of main (4b37350e6, carrying d20c7f847 "docs(shell): collapse shell guidance into one Rules section") rewrote the shell tool's `description.md`, replacing the old text with new markers — "Do not use grep to search files", "Do not use shell to edit files". That made the test's own marker list stale. A separate stale-assertion fix, bundled into 59bd9ae5c, updated the marker list in `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` to match. The file was read directly to confirm, not just the diff.

    So this card's premise held when it was written, and the description did not "lose" its guidance — the guidance moved and the test lagged. Left open for a person to close.
  timestamp: 2026-08-12T11:34:51.909823+00:00
position_column: todo
position_ordinal: ffd980
title: 'shell_description_states_blocking_and_no_tail fails on main: the shell tool description lost its grep-files guidance'
---
`swissarmyhammer-tools mcp::tools::shell::tests::shell_description_states_blocking_and_no_tail` fails on main.

The test asserts that `McpTool::description(&ShellExecuteTool)` contains `"grep files"` and ``"never `grep -r`"``. The description text in `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` (the function near line 591) holds neither string.

## Measured on 2026-08-12

Found during the test step of ^nkyb681. `git stash` removes that card's diff, and the failure reproduces identically on the unchanged tree, so the failure is pre-existing and unrelated.

It is not one of the four failures carded on ^bh5ncd0.

## What to do

Decide which side is correct — the test states a requirement the description must meet, or the description moved and the test holds a stale string. Then make the two agree.

#test-failure