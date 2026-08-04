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
- actor: claude-code
  id: 01kz5ak0e5y9kmw28ry8xp4mfk
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — CreateAgentOptions gained extra_claude_args/skip_init_trigger; review_create_agent_options() is the single source of truth for --strict-mcp-config + --disable-slash-commands + skip_init_trigger:true, used by review_agent_factory. ClaudeConfig/SpawnConfig carry skip_init_trigger (default false). spawn_process_and_consume_init short-circuits to Ok((None,None)) when set, skipping both the "hi" trigger write and the blocking init read. Composes cleanly with ^j9rwjtx/^1hmd9yy's fork-extra_args fix. Spot-checked all four new symbols present in the actual diff.
    - test: green — claude-agent 763 passed, swissarmyhammer-agent 73 passed, review pipeline 193+193 passed; fmt clean; clippy clean; full rdeps(claude-agent) or rdeps(swissarmyhammer-agent) run for real: 4404 passed, 0 skipped
    - commit: ddedbc319
    - next: /review HEAD~1..HEAD
  timestamp: 2026-08-04T02:45:34.277838+00:00
- actor: claude-code
  id: 01kz5c7gp68pv5afmebeczp0ew
  text: |-
    ### review — findings
    - evidence: 3 findings after blame-check vs ddedbc319 — crates/claude-agent/src/agent.rs:1618, crates/claude-agent/src/claude.rs:1970, crates/swissarmyhammer-agent/src/lib.rs:762 (engine reported 71 confirmed findings; 68 dropped as pre-existing code untouched by this commit per blame)
    - next: implement the 3 findings, then re-review
  timestamp: 2026-08-04T03:14:14.854551+00:00
- actor: claude-code
  id: 01kz5c8pn0d2ev8q151p77vk14
  text: |-
    ### finish iteration 1 — findings
    - review: findings — engine ran successfully (71 raw, 68 blame-checked out as pre-existing). 3 survive: (1) agent.rs:1618 — skip_init_trigger has no session-persistence step (unlike extra_args's store_extra_args_in_session), so build_fork_spawn_config never carries it into forks — the exact same gap class ^j9rwjtx round 2 fixed for extra_args; (2) claude.rs:1970 — new test's polling loop uses a hardcoded bound of 40, no named constant; (3) lib.rs:762 — skip_init_trigger set as a separate line after build_claude_agent_config() returns, breaking the established pattern where ephemeral/tools_override/auto_allow_all/extra_args are all threaded as parameters and set inside it.
    - next: /implement — add Session.skip_init_trigger + store_skip_init_trigger_in_session (or reuse the shared persist_session_field helper from ^j9rwjtx round 3) and carry it in build_fork_spawn_config from parent.skip_init_trigger; name the test's 40 constant; move skip_init_trigger to a build_claude_agent_config parameter matching the other fields
  timestamp: 2026-08-04T03:14:53.728235+00:00
- actor: claude-code
  id: 01kz5ctmbyaa0fp7qhm0j405hq
  text: |-
    ### implement — changed

    Fixed all 3 review findings from the 2026-08-03 21:45 batch:

    1. `crates/claude-agent/src/agent.rs` — `skip_init_trigger` had no session-persistence step for fork replay.
       - Added `skip_init_trigger: bool` field to `Session` (`crates/claude-agent/src/session.rs`), `#[serde(default)]`, initialized `false` in `Session::new`, mirroring `extra_args`.
       - Added `store_skip_init_trigger_in_session()` in `agent.rs`, using the shared `persist_session_field` helper (not a fourth near-duplicate `store_*` function), mirroring `store_extra_args_in_session`.
       - Called it from `create_new_session_internal()` right after `store_extra_args_in_session(&session_id)?;`.
       - `crates/claude-agent/src/session_fork.rs`'s `build_fork_spawn_config` now reads `.skip_init_trigger(parent.skip_init_trigger)` instead of the live `self.config.claude.skip_init_trigger`.
       - Added test `test_fork_spawn_config_carries_parent_skip_init_trigger_not_live_config` in `session_fork.rs`, mirroring `test_fork_spawn_config_carries_parent_extra_args_not_live_config`: sets the live config's `skip_init_trigger` to `false`, sets the parent session's `skip_init_trigger` to `true` directly, forks, and asserts the spawn config carries the PARENT's `true`, not the live config's `false`.

    2. `crates/claude-agent/src/claude.rs` — extracted `const CAPTURE_FILE_POLLING_ATTEMPTS: usize = 40;` in the test module, documenting it bounds the wait (2s at 50ms/attempt) for the fake `claude` script's stdin-capture file to appear in `spawn_process_and_consume_init_skips_trigger_when_configured`. Replaced the hardcoded `0..40` with `0..CAPTURE_FILE_POLLING_ATTEMPTS`.

    3. `crates/swissarmyhammer-agent/src/lib.rs` — `build_claude_agent_config()` now takes `skip_init_trigger: bool` as a parameter (after `extra_args`), set inside the function body alongside `ephemeral`/`tools_override`/`extra_args` (`agent_config.claude.skip_init_trigger = skip_init_trigger;`), instead of being set on a separate line after the call returns in `claude_agent_config_from_model`. Updated all 8 call sites (production + tests) to pass the new parameter. Added test `test_build_claude_agent_config_threads_skip_init_trigger` proving both `true` and `false` thread through.

    Test counts:
    - `cargo test -p claude-agent --lib`: 764 passed, 0 failed (763 + 1 new)
    - `cargo test -p swissarmyhammer-agent --lib`: 74 passed, 0 failed (73 + 1 new)
    - `cargo fmt --all`: no diff beyond the edits above
    - `cargo clippy --workspace --all-targets -- -D warnings`: clean
    - `cargo nextest run -E 'rdeps(claude-agent) or rdeps(swissarmyhammer-agent)'`: 4406 tests run, 4406 passed, 0 skipped

    next: /review
  timestamp: 2026-08-04T03:24:41.214759+00:00
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
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Review Findings (2026-08-03 21:45)

- [x] `crates/claude-agent/src/agent.rs:1618` — The diff adds `.skip_init_trigger()` to spawn config builders (lines 1618 and 1927), reading from `self.config.claude.skip_init_trigger`. This mirrors the pattern established by `extra_args`: both are config-driven spawn parameters. However, `extra_args` is also persisted onto the session at line 1374 via `store_extra_args_in_session()` for later `session/fork` replay (see comment at lines 1370–1374). The diff does not add a corresponding storage step for `skip_init_trigger` in `create_new_session_internal()`. This breaks the invariant that spawn config values driven by agent config are stored on the session so a fork can replay the parent's frozen values, not the agent's live config at fork time. Add `store_skip_init_trigger_in_session(&session_id)?;` in `create_new_session_internal()` after line 1374, to persist the spawn-time value on the session for fork replay. If `Session` lacks a `skip_init_trigger` field, add it first and create a `store_skip_init_trigger_in_session()` helper method following the pattern of `store_extra_args_in_session()`.
- [x] `crates/claude-agent/src/claude.rs:1970` — Hardcoded loop bound 40 in test polling should be a named constant to document the maximum polling attempts and make it adjustable. Define a named constant (e.g., `const CAPTURE_FILE_POLLING_ATTEMPTS: usize = 40;`) and use it instead of the inline literal.
- [x] `crates/swissarmyhammer-agent/src/lib.rs:762` — spawn configuration options are handled inconsistently: `ephemeral`, `tools_override`, `auto_allow_all`, and `extra_args` are all passed as parameters to `build_claude_agent_config()` and set within it (lines 819-821), but `skip_init_trigger` is set AFTER the function returns (line 762). This breaks the pattern and could lead to bugs if the function is called directly elsewhere and the caller forgets to set this field. Add `skip_init_trigger: bool` as a parameter to `build_claude_agent_config()` (between line 793 and the function body), then set it within the function alongside `ephemeral` and `tools_override` (around line 819-821). This makes all spawn-related options consistent. #review