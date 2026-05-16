---
assignees:
- claude-code
position_column: todo
position_ordinal: 8d80
title: Make `sah agent acp` serve Claude Code, not just local llama
---
## What
Standalone defect, independent of the AI panel. `apps/swissarmyhammer-cli/src/commands/agent/acp.rs` hardcodes the local-llama backend: it builds `llama_agent::AgentServer` + `llama_agent::acp::AcpServer` and can only run a local llama model as an ACP agent. There is no way to run Claude Code (claude-agent) through `sah agent acp` — yet `sah agent acp` is the ACP entrypoint for editor integration (Zed, etc.) and should serve whichever model the user configured.

- Route `sah agent acp` through `swissarmyhammer_agent::create_agent(&model_config, ...)` — which already dispatches `ModelExecutorType::ClaudeCode` vs `LlamaAgent` — instead of instantiating `llama_agent` directly.
- The model comes from the ACP `--config` file (or the configured default model). The existing `acp.rs` options — `--config`, `--permission-policy`, `--allow-path`/`--block-path`, `--max-file-size`, `--terminal-buffer-size`, `--graceful-shutdown-timeout` — must keep working.
- Serve the resulting agent over stdio as today (`start_with_streams(stdin(), stdout())`).

## Acceptance Criteria
- [ ] `sah agent acp` with a Claude Code model config runs claude-agent as the ACP server over stdio.
- [ ] `sah agent acp` with a local llama model config still works as before.
- [ ] All existing `acp.rs` CLI options still apply to both backends.
- [ ] `cargo build -p swissarmyhammer-cli` is clean.

## Tests
- [ ] Integration test: `sah agent acp` with a Claude Code config — connect an ACP client, assert `initialize` succeeds against a claude-backed agent.
- [ ] Integration test: the local-llama path still completes `initialize` unchanged.
- [ ] `cargo test -p swissarmyhammer-cli` is green.

## Workflow
- Use `/tdd` — write the Claude-Code `sah agent acp` integration test first. #bug