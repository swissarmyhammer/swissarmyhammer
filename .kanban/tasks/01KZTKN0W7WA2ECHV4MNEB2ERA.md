---
assignees:
- claude-code
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