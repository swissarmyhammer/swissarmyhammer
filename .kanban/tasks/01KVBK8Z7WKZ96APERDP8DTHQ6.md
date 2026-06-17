---
assignees:
- claude-code
position_column: todo
position_ordinal: a280
project: kv-prefix-reuse
title: Guarantee deterministic tools/list ordering for the agent-tools MCP mount
---
## What
The shared-prefix donor only stays valid if the rendered prefix (system prompt + tool definitions) is byte-identical across sibling review sessions. The system prompt is already a static string, and llama-agent passes tools as order-preserving `Vec`s — but the intrinsic agent-tools MCP mount in `swissarmyhammer-tools` is the one place a tool list could be built from a `HashMap`, making `tools/list` order vary run-to-run and silently shrinking the LCP below the full prefix (the divergence diagnostic at `crates/llama-agent/src/queue.rs:2582` exists to catch exactly this).

Audit and guarantee deterministic tool ordering:
- Find where the agent-tools MCP server builds its `tools/list` response in `swissarmyhammer-tools` (search the tool router / `list_tools` / tool registry; e.g. `swissarmyhammer-tools/src/mcp/`). Confirm whether iteration order is stable (Vec / BTreeMap / IndexMap) or `HashMap`-based.
- If non-deterministic, sort by a stable key (tool name) or switch the backing collection to a deterministic one at the emission point. Do not change tool identity or routing (the routing `tool_index` HashMap at `agent.rs:119` is fine — only the rendered LIST order matters).

## Acceptance Criteria
- [ ] `tools/list` from the agent-tools mount returns tools in a deterministic, stable order across repeated calls and process restarts.
- [ ] Tool routing/dispatch behavior is unchanged.

## Tests
- [ ] Add a unit/integration test in `swissarmyhammer-tools` asserting two successive `list_tools` calls return identical ordered name sequences, and that the order matches an expected stable ordering (e.g. sorted by name).
- [ ] If a HashMap was the source, add a test that would fail under hash randomization (assert exact order, not set-equality).
- [ ] `cargo test -p swissarmyhammer-tools` green.

## Workflow
- Use `/tdd` — write the ordering assertion first; if it's already deterministic the test is a regression guard (note that in the task) and no code change is needed beyond the test.