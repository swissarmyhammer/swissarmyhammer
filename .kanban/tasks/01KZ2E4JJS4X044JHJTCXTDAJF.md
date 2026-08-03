---
assignees:
- claude-code
depends_on:
- 01KZ23NHE9JR2WAMMQP3Y5N9G6
position_column: todo
position_ordinal: ee80
project: drop-llama-agent
title: Refresh ARCHITECTURE.md for the Claude-only agent facade
---
## What

`ARCHITECTURE.md` still describes two chat backends. After the llama-agent
crate is gone, these statements are wrong:

- The `swissarmyhammer-agent (facade)` diagram lists a `llama-agent` branch
  under `create_agent(ModelConfig) dispatches to:`.
- The paragraph after that diagram says "`ModelConfig` determines the backend
  — the `executor_type` field selects `ClaudeCode` or `LlamaAgent`."
  `create_agent` now builds Claude only and rejects every other executor by
  name.
- The end-of-turn-marker paragraph names `llama-agent`,
  `llama_agent::AcpServer`, and `wrap_llama_into_handle`. Only
  `wrap_claude_into_handle` is left.
- The Patterns list has a **Dual-Backend, Single Protocol** entry.

Do NOT touch the embedding statements. **Platform-Aware Embedding** is still
true and the embedding stack stays.

`ARCHITECTURE.md` is written by the `map` skill, so regenerate it with `/map`
rather than hand-editing, then read the result and correct anything the
regeneration got wrong.

### Subtasks

- [ ] Regenerate `ARCHITECTURE.md` with `/map`.
- [ ] Confirm no `llama-agent` chat backend statement is left.
- [ ] Confirm the embedding statements are unchanged.

## Acceptance Criteria

- [ ] `ARCHITECTURE.md` describes exactly one chat backend, claude-agent.
- [ ] `ARCHITECTURE.md` still describes the embedding stack.

## Workflow

- Documentation only, no code. The proof is reading the regenerated file. #bug #cleanup #docs #llama-agent