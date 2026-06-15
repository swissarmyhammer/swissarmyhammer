---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffab80
project: claude-model-select
title: Log the resolved review/agent model so the tier is provable
---
## What
We currently cannot prove which Claude model (tier) a review run actually used. `claude-agent` already logs the spawn argv (`crates/claude-agent/src/claude_process.rs::log_command`, the `🚀 Spawning Claude CLI` line at `info`, which includes `--model haiku` when present), but (a) nothing logs the *resolved review model* at the decision point, and (b) the claude-agent subprocess tracing doesn't land in the `.sah` debug logs operators actually read, so the tier is invisible.

Add decision-point logging and make the spawn argv observable:
- `apps/swissarmyhammer-cli/src/commands/serve/mod.rs::review_model_config` — when a review model is resolved, `tracing::info!` the resolved model **name + executor type** (e.g. "review scope → claude-code-haiku (ClaudeCode)"). Also log when it returns `None` (global fallback).
- `crates/swissarmyhammer-agent/src/lib.rs` — in the claude agent build path (`create_agent_with_options` / `build_claude_agent_config`), log the resolved model name/executor and the `extra_args` being passed (so the chosen tier is recorded before the subprocess even starts).
- Investigate why `claude-agent`'s `log_command` `info` line does not reach the `.sah` logs (subprocess tracing routing) and make the spawn argv visible there — either by ensuring the claude-agent ACP subprocess shares the `.sah` tracing subscriber/log file, or by logging the argv from the parent at the point it launches/serves the connection. Do NOT use `eprintln!` (stderr is swallowed by MCP — see project memory `use-tracing`).

## Acceptance Criteria
- [ ] A real review run records, in the `.sah` logs, which model name + executor the review scope resolved to.
- [ ] The claude CLI start args (argv, including any `--model`) are visible in the `.sah` logs for a review run, not only in the (swallowed) subprocess stderr.
- [ ] No `eprintln!`/`println!` added for this; all via `tracing`.

## Tests
- [ ] `crates/swissarmyhammer-agent` or `apps/swissarmyhammer-cli`: a test using `tracing-test` (or capturing subscriber) asserting that resolving a `claude-code-haiku` review model emits a log line containing the resolved name `claude-code-haiku`.
- [ ] `crates/claude-agent`: assert (via captured tracing) that `log_command`/spawn path emits the argv including `--model haiku` when `extra_args` carries it. Reuse the existing `build_base_command` arg-inspection tests as the seam.
- [ ] Run: `cargo test -p swissarmyhammer-agent -p claude-agent -p swissarmyhammer-cli` — all green.

## Workflow
- Use `/tdd` — write the failing log-capture test first, then implement. #haiku

## Review Findings (2026-06-14 13:40)

Strong, well-tested change. All four verification points hold: logging is tracing-only, the log-capture tests are mutation-sensitive (each `logs_contain` substring is produced only by the new log statement, so deleting the log fails the test), the flat argv line makes `--model haiku` greppable as one contiguous record, and tests pass (`swissarmyhammer-agent`, `claude-agent`, and all 7 `review_model_config` CLI tests — the emitted lines `review scope → claude-code (ClaudeCode)`, `Building claude agent (executor=ClaudeCode, extra_args=[\"--model\", \"haiku\"])`, and the single-line `🚀 Spawning Claude CLI argv: … --model haiku` were observed in test output).

### Nits
- [x] `crates/claude-agent/src/claude_process.rs` (`log_command`) — The new single-line `🚀 Spawning Claude CLI argv: <space-joined argv>` is emitted at INFO after `configure_system_prompt` runs, so when a system prompt is set the entire prompt is logged inline on one INFO line. This is pre-existing behavior in kind — the adjacent `Pretty` block already logged the full args (including `--system-prompt <prompt>`) at INFO — and the prompt is the SAH system prompt, not a credential (no API keys/tokens appear in argv), so this is not a blocker. Consider redacting/eliding `--system-prompt`'s value (e.g. replace the value following `--system-prompt` with `<NNN chars>`) when building the flat argv string, so the greppable tier line stays compact and a large prompt is not duplicated at INFO. Optional given it does not worsen the existing exposure.

**Resolved (2026-06-14):** Added shared `redact_system_prompt_value` helper in `log_command`; both the flat `🚀 Spawning Claude CLI argv:` line and the adjacent `Pretty` block now consume the redacted args from one code path. The `--system-prompt` flag stays visible; its value is replaced with `<system-prompt: N chars>`. `--model haiku` and all other args remain fully visible (tier stays greppable). Tracing only. Added TDD test `test_log_command_redacts_system_prompt_value` (red→green verified) asserting the flat line omits the prompt body but keeps `--model haiku`, `--system-prompt`, and the placeholder. Existing greppability tests stay green. `cargo test -p claude-agent -p swissarmyhammer-agent -p swissarmyhammer-cli` and `cargo clippy ... -- -D warnings` both clean.