---
assignees:
- claude-code
position_column: todo
position_ordinal: e080
title: Spawn review subagent Claude CLI with no MCP servers, no skills, and no init turn
---
## What

The review engine spawns one `claude` CLI process for each validator fork and each verify agent (~45 processes for each run). Each process is heavier than necessary in three ways:

1. The spawn does not pass `--strict-mcp-config` when `SpawnConfig.mcp_servers` is empty (`configure_mcp_servers` returns early at `crates/claude-agent/src/claude_process.rs:583`). The CLI then loads the user MCP configuration: each review subagent boots its own sah MCP server and the chrome-devtools plugin server. The `.sah/` folder shows one `mcp.<pid>.log` boot log for each spawn.
2. The CLI loads all skills and slash commands (the init message reports 70 slash commands and 23 skills). Validators do not use them.
3. `spawn_process_and_consume_init` (`crates/claude-agent/src/claude.rs:316`) sends a "hi" init-trigger turn (`claude.rs:354`) to make the CLI emit its `system/init` message. This turn is one full API call (~300 output tokens) for each spawn. The review engine does not use the agent list or the slash-command list from that init message.

Make these changes:

- In `review_agent_factory` (`crates/swissarmyhammer-agent/src/lib.rs:431`), append `--strict-mcp-config` and `--disable-slash-commands` to the `extra_args` that flow into `claude_agent::AgentConfig` (the seam is `claude_agent_config_from_model` → `build_claude_agent_config`). `--strict-mcp-config` with no `--mcp-config` gives zero MCP servers. `--disable-slash-commands` disables all skills.
- Add a `skip_init_trigger: bool` field to `ClaudeConfig` (`crates/claude-agent/src/config.rs:92`) and `SpawnConfig` (`crates/claude-agent/src/claude_process.rs:110`), default `false`. When it is `true`, `spawn_process_and_consume_init` must not send the "hi" trigger and must not block on the init read. The stream loop must then absorb the `type=system, subtype=init` line when it comes before the first real prompt response.
- Set `skip_init_trigger: true` in `review_agent_factory` only. All other paths (kanban-app, agent builtins) keep the current behavior.

## Subtasks

- [ ] Append `--strict-mcp-config` and `--disable-slash-commands` to review `extra_args` in `review_agent_factory`
- [ ] Add `skip_init_trigger` to `ClaudeConfig` and `SpawnConfig`; make the "hi" trigger and the blocking init read conditional
- [ ] Make the stream loop skip a late `system/init` line when `skip_init_trigger` is set
- [ ] Set `skip_init_trigger: true` in `review_agent_factory`
- [ ] Add the tests below

## Acceptance Criteria

- [ ] The review agent config carries `--strict-mcp-config` and `--disable-slash-commands` in `extra_args`, after the existing `--model haiku` args
- [ ] A review spawn writes no "hi" init trigger to the CLI stdin
- [ ] Default (non-review) spawns keep the init trigger and do not get the new flags
- [ ] The existing review pipeline tests pass unchanged (`cargo test -p swissarmyhammer-validators` and `cargo test -p swissarmyhammer-tools review`)

## Tests

- [ ] Config-seam unit test in `crates/swissarmyhammer-agent/src/lib.rs` (next to `review_resolved_default_spawns_claude_with_model_haiku`, ~line 2489): assert the resolved review agent config `extra_args` contains `--strict-mcp-config` and `--disable-slash-commands`, and `skip_init_trigger` is `true`
- [ ] Unit test in `crates/claude-agent/src/claude_process.rs`: a `SpawnConfig` with empty `mcp_servers` plus the new extra args produces an argv with `--strict-mcp-config` and `--disable-slash-commands`
- [ ] Unit test in `crates/claude-agent/src/claude.rs`: with `skip_init_trigger: true`, `spawn_process_and_consume_init` writes no init trigger line to the process stdin
- [ ] Run `cargo test -p claude-agent -p swissarmyhammer-agent` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #review