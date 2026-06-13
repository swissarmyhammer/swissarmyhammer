---
assignees:
- claude-code
position_column: todo
position_ordinal: 9f80
project: claude-model-select
title: Wire ClaudeCodeConfig.args from ModelConfig into the claude agent builder
---
## What
`ClaudeCodeConfig` already has a `pub args: Vec<String>` field (`crates/swissarmyhammer-config/src/model.rs` ~line 400) and an optional `claude_path: Option<PathBuf>`, but nothing consumes them. This task threads those values from the resolved `ModelConfig` into the claude agent so the spawn-path plumbing from the prior task (`SpawnConfig.extra_args`) actually receives the YAML-configured switches.

Files:
- `crates/swissarmyhammer-agent/src/lib.rs`
  - `create_agent_with_options` (~line 367), `ClaudeCode` arm (~line 372): extract the `ClaudeCodeConfig` (args + claude_path) from the `ModelConfig` and pass it into `create_claude_agent` (~line 808).
  - `build_claude_agent_config` (~line 853): set the new `ClaudeConfig.extra_args` (and `claude_path` if present) from the `ClaudeCodeConfig`, in addition to the existing `ephemeral`/`tools_override` handling.
- Depends on `ClaudeConfig.extra_args` existing (prior task) and on `SpawnConfig` consuming it.

Note: `ClaudeCodeConfig.claude_path` is also currently dropped; wire it through to the existing `claude_path` resolution if there is one, otherwise leave a clearly-commented TODO and scope this task to `args` only. Confirm during implementation whether `claude_path` already has a sink before expanding scope.

## Acceptance Criteria
- [ ] A `ModelConfig` whose `claude-code` executor has `args: ["--model", "haiku"]` produces a `ClaudeConfig` with `extra_args == ["--model", "haiku"]`.
- [ ] The args reach the spawned command (end-to-end with the prior task's plumbing).
- [ ] A `claude-code` model with empty `args` behaves exactly as today.

## Tests
- [ ] In `crates/swissarmyhammer-agent` tests: construct a `ModelConfig` for `claude-code` with `args: ["--model", "haiku"]`, run it through `build_claude_agent_config`, and assert the resulting `ClaudeConfig.extra_args` equals `["--model", "haiku"]`.
- [ ] A test asserting empty `args` yields empty `extra_args` (regression guard).
- [ ] If feasible without spawning a real `claude` process, an integration-style test asserting the built `SpawnConfig.extra_args` carries the values through. If a real spawn is required, gate it behind the existing claude-cli test guard rather than running unconditionally.
- [ ] Run: `cargo test -p swissarmyhammer-agent` — all green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.