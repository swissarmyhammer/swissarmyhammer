---
assignees:
- claude-code
position_column: todo
position_ordinal: e780
title: llama-agent real-model integration tests time out at 300s in this sandbox — pre-existing, unrelated to ^6jsxjbc
---
## What happened

While verifying `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` for `^6jsxjbc`
(the review batch budget fix), two `llama-agent` real-model integration tests timed out on
every one of their 3 tries (the package's configured `retries = 2`), even with no other
`cargo nextest` process running concurrently:

- `llama-agent::agent_tests integration::agent_tools_mount::agent_tools_mount_lists_intrinsic_tools_with_no_external_servers`
- `llama-agent::agent_tests integration::dual_source_shell_dedup::llama_dual_source_aggregation_has_shell_exactly_once`

Both hit exactly the package's 300s `slow-timeout` (`terminate-after = 10` at `period = "30s"`)
on every try. Full run: `5059 tests run: 5057 passed (1 slow), 2 timed out, 2 skipped`.

## Why this is NOT ^6jsxjbc

Neither test references `claude_agent`, `swissarmyhammer_validators`, `swissarmyhammer_agent`,
or `MAX_PROMPT_LENGTH` — confirmed with a grep of both test files. `llama-agent` is a wholly
separate ACP backend from `claude-agent`; nothing in the `^6jsxjbc` diff touches
`crates/llama-agent`. The two tests exercise llama-agent's own real-model tool-mounting and
shell-tool dedup, unrelated to prompt-length budgeting.

## Evidence this is environmental, not flaky-as-usual

The nextest config already documents these real-model tests as nondeterministic under GPU
contention (hence the bounded `retries = 2`). But this run hit the SAME 300s wall on every
try, including a rerun with zero other `cargo nextest` processes on the machine — a
deterministic hang, not the documented contention-driven variance. Worth investigating
whether a specific real-model call in these two tests is genuinely stuck (e.g. a hung
generation, a model load that never completes) rather than just slow.

## Suggested next step

Reproduce standalone: `cargo nextest run -p llama-agent -E 'test(agent_tools_mount_lists_intrinsic_tools_with_no_external_servers) + test(llama_dual_source_aggregation_has_shell_exactly_once)' --no-capture` and inspect where each hangs.
#bug #test