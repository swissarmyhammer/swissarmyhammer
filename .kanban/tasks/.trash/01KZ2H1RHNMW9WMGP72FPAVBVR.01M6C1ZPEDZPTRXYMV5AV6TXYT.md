---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz2j5s2a0yx2q6bba8f35ceb
  text: |-
    ### The native-KV review path is the same question

    While scrubbing prose for ^3y5n9g6 I found a second thing orphaned by the same
    deletion. Fold it into this card's decision.

    `swissarmyhammer-validators`'s review fleet classifies a prefix reuse as
    `ReuseKind::WarmKv` when the backend reports a native KV fork carrying
    `fork.prefix_tokens`. `PoolConfig::local()` (one worker) exists for the same
    backend. llama-agent was the only agent that reported a native KV cache;
    claude-agent has none and its pin is a documented no-op.

    So `WarmKv` and the one-worker local policy are now exercised only by the mock
    backends in `crates/swissarmyhammer-validators/src/review/test_support.rs`. The
    prose in `fleet.rs`, `pool.rs`, `drive.rs`, and `fleet/tests.rs` was reworded
    from "the llama/qwen backend" to "a backend with a native KV cache" — true, and
    crate-free, but it does not answer whether any such backend can still connect.

    Same shape as `create_agent_tools_server`: a capability with no production
    consumer left. Decide both together — keep the seam for a future in-process
    agent, or remove the seam and its mocks.
  timestamp: 2026-08-03T01:00:26.058613+00:00
position_column: todo
position_ordinal: f180
project: drop-llama-agent
title: Decide the fate of the now-uncalled agent-tools mount
---
## What

`McpServer::create_agent_tools_server` in
`crates/swissarmyhammer-tools/src/mcp/server.rs` has zero callers.

It built the `Agent`-category registry (files, web, skill, subagent) plus the
shell `Replacement` tool as a second `McpServer` with
`compose_per_client = false`. Its only consumer was
`build_agent_tools_mount` in `apps/swissarmyhammer-cli/src/commands/agent/acp.rs`,
which wrapped it in `llama_agent::InProcessMount` and handed it to every
llama-agent ACP session. Card ^3y5n9g6 deleted that command with the crate.

`ToolCategory::Agent` is the matching hole: `Host::serves` returns `false` for
that category for EVERY host, so an `Agent`-category tool now reaches nobody.
The mount was the one path that served them.

Decide and act:

- Keep it as public API for a future in-process agent, or
- Delete `create_agent_tools_server`, and then decide whether
  `ToolCategory::Agent` still earns its place or collapses into the
  Shared/Replacement split.

Do not do half of it. A public function with no caller and a tool category
that reaches no host are the same question.

### Subtasks

- [ ] Decide keep or delete.
- [ ] If delete: remove `create_agent_tools_server` and re-check every
      `ToolCategory::Agent` tool's category.

## Acceptance Criteria

- [ ] Either `create_agent_tools_server` has a caller, or it is gone.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

## Tests

- [ ] Run `cargo nextest run -p swissarmyhammer-tools` — the per-client
      composition tests still pass.
- [ ] Run `cargo nextest run --workspace`. #bug #cleanup #llama-agent