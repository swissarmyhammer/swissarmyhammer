---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa780
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

RESOLUTION: Confirmed `claude_path` has NO sink anywhere in `crates/claude-agent` (zero grep matches). Scoped task to `args` only; left a clearly-commented TODO at the extraction site in `create_agent_with_options`.

## Acceptance Criteria
- [x] A `ModelConfig` whose `claude-code` executor has `args: ["--model", "haiku"]` produces a `ClaudeConfig` with `extra_args == ["--model", "haiku"]`.
- [x] The args reach the spawned command (end-to-end with the prior task's plumbing).
- [x] A `claude-code` model with empty `args` behaves exactly as today.

## Tests
- [x] In `crates/swissarmyhammer-agent` tests: construct a `ModelConfig` for `claude-code` with `args: ["--model", "haiku"]`, run it through `build_claude_agent_config`, and assert the resulting `ClaudeConfig.extra_args` equals `["--model", "haiku"]`.
- [x] A test asserting empty `args` yields empty `extra_args` (regression guard).
- [x] Integration-style test asserting the args flow from `ModelConfig` through `config.executor()` extraction into `build_claude_agent_config` -> `ClaudeConfig.extra_args`, where the existing spawn-path plumbing carries them through (no real spawn required).
- [x] Run: `cargo test -p swissarmyhammer-agent` — all green (86 passed, 0 failed in lib; integration suites green).

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #haiku

## Review Findings (2026-06-13 15:30)

### Warnings
- [x] `crates/swissarmyhammer-agent/src/lib.rs:380-384` and `:2388-2392` — The `ClaudeCode` extraction (`if let ModelExecutorConfig::ClaudeCode(cfg) = config.executor() { cfg.args.clone() } else { Vec::new() }`) lived only inside `create_agent_with_options` and was hand-duplicated by the test. RESOLVED: extracted a private helper `fn claude_code_args(config: &ModelConfig) -> Vec<String>` (lib.rs ~line 371) — the single source of truth for the ClaudeCode arg lookup. Both the production `create_agent_with_options` ClaudeCode arm (`let extra_args = claude_code_args(config);`) and the test `test_claude_code_config_args_flow_from_model_config` (`let extra_args = claude_code_args(&config);`) now call it, so the test pins the real production code path and flipping the helper (e.g. to `Vec::new()`) breaks the assertion. Verified: `cargo test -p swissarmyhammer-agent` 86 lib + integration suites green, 0 failures; `cargo clippy -p swissarmyhammer-agent --all-targets -- -D warnings` 0 warnings.