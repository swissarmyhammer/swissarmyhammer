---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz5af6favawhdcgfq43jd0sp
  text: |-
    ### implement — changed

    Files touched:
    - `crates/swissarmyhammer-agent/src/lib.rs` — `CreateAgentOptions` gets `extra_claude_args: Vec<String>` and `skip_init_trigger: bool`. `claude_agent_config_from_model` appends `options.extra_claude_args` after `config.claude_args()` and sets `agent_config.claude.skip_init_trigger = options.skip_init_trigger`. New `review_create_agent_options()` (extra_claude_args = `["--strict-mcp-config", "--disable-slash-commands"]`, skip_init_trigger = true) is the single source `review_agent_factory` and its test both use — factory now calls `create_agent_with_options(&config, None, review_create_agent_options())`. Added test `review_resolved_config_disables_mcp_skills_and_init_trigger` next to `review_resolved_default_spawns_claude_with_model_haiku`.
    - `crates/claude-agent/src/config.rs` — `ClaudeConfig.skip_init_trigger: bool` (`#[serde(default)]`), wired into `AgentConfig::default()`.
    - `crates/claude-agent/src/claude_process.rs` — `SpawnConfig.skip_init_trigger: bool` (`#[builder(default)]`). Added test `test_extra_args_carry_review_flags_with_empty_mcp_servers`.
    - `crates/claude-agent/src/claude.rs` — `spawn_process_and_consume_init` reads `config.skip_init_trigger` before the process is consumed by `spawn_process`; when true, returns `Ok((None, None))` immediately without `send_init_trigger` or `read_init_message`. Added test `spawn_process_and_consume_init_skips_trigger_when_configured`, which shims `PATH` (RAII `PathGuard`, `#[serial_test::serial(path_env)]`) to a fake `claude` script that captures stdin, proving no line is written when `skip_init_trigger: true`.
    - `crates/claude-agent/src/agent.rs` — both `build_session_spawn_config` and `build_mode_spawn_config` now thread `.skip_init_trigger(self.config.claude.skip_init_trigger)` onto the `SpawnConfig`, alongside the existing `ephemeral`/`tools_override`/`extra_args` carry-through — matching the `^j9rwjtx`/`^1hmd9yy` extra_args propagation fix already in place, composes without conflict.

    Verified by code inspection (no change needed): the stream loop's `process_notification` already has a catch-all `_ => true` arm for any `SessionUpdate` variant other than `AgentMessageChunk`/`ToolCall`/`ToolCallUpdate`. A late `system/init` line (if the CLI emits one before the first real prompt response) parses to `SessionUpdate::AvailableCommandsUpdate` via `handle_system_init`, which already falls into that catch-all and is silently absorbed — and with `--disable-slash-commands` set alongside `skip_init_trigger` for review, there's no `slash_commands` field in the init line at all, so `handle_system_init` returns `Ok(None)` and the line is skipped outright.

    Test counts:
    - `cargo test -p claude-agent --lib`: 763 passed, 0 failed
    - `cargo test -p swissarmyhammer-agent --lib`: 73 passed, 0 failed
    - `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools review`: 193 + 193 passed, 0 failed
    - `cargo fmt --all`: no diff beyond the edits above
    - `cargo clippy -p claude-agent -p swissarmyhammer-agent --all-targets -- -D warnings`: clean
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean
    - `cargo nextest run -E 'rdeps(claude-agent) or rdeps(swissarmyhammer-agent)'`: 4404 tests run, 4404 passed, 0 skipped

    next: /review
  timestamp: 2026-08-04T02:43:29.386242+00:00
position_column: doing
position_ordinal: '8380'
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

- [x] Append `--strict-mcp-config` and `--disable-slash-commands` to review `extra_args` in `review_agent_factory`
- [x] Add `skip_init_trigger` to `ClaudeConfig` and `SpawnConfig`; make the "hi" trigger and the blocking init read conditional
- [x] Make the stream loop skip a late `system/init` line when `skip_init_trigger` is set
- [x] Set `skip_init_trigger: true` in `review_agent_factory`
- [x] Add the tests below

## Acceptance Criteria

- [x] The review agent config carries `--strict-mcp-config` and `--disable-slash-commands` in `extra_args`, after the existing `--model haiku` args
- [x] A review spawn writes no "hi" init trigger to the CLI stdin
- [x] Default (non-review) spawns keep the init trigger and do not get the new flags
- [x] The existing review pipeline tests pass unchanged (`cargo test -p swissarmyhammer-validators` and `cargo test -p swissarmyhammer-tools review`)

## Tests

- [x] Config-seam unit test in `crates/swissarmyhammer-agent/src/lib.rs` (next to `review_resolved_default_spawns_claude_with_model_haiku`, ~line 2489): assert the resolved review agent config `extra_args` contains `--strict-mcp-config` and `--disable-slash-commands`, and `skip_init_trigger` is `true`
- [x] Unit test in `crates/claude-agent/src/claude_process.rs`: a `SpawnConfig` with empty `mcp_servers` plus the new extra args produces an argv with `--strict-mcp-config` and `--disable-slash-commands`
- [x] Unit test in `crates/claude-agent/src/claude.rs`: with `skip_init_trigger: true`, `spawn_process_and_consume_init` writes no init trigger line to the process stdin
- [x] Run `cargo test -p claude-agent -p swissarmyhammer-agent` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #review