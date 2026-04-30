---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
project: acp-upgrade
title: 'ACP 0.11: Update ARCHITECTURE.md to reflect builder/handler API'
---
## What

`ARCHITECTURE.md:493` currently says "New agent backends must implement `Agent` from `agent-client-protocol`." That sentence describes the 0.10 trait-based contract. In ACP 0.11, `Agent` is a unit struct and backends register handlers on `Agent.builder()`. The doc should be updated to match the new model once the broader 0.11 migration lands across all agent backends.

This was originally flagged as a nit on task 01KQD0NNG9DPHWATDNN61EERE2 ("ACP 0.11: llama-agent: acp/server.rs (AcpServer reshape)") but explicitly marked out of scope for that single-file change. Spun off here for follow-up.

## Files

- `ARCHITECTURE.md` (line 493 specifically; nearby section may also need a refresh)

## Acceptance Criteria

- [ ] Sentence at `ARCHITECTURE.md:493` updated to describe the 0.11 builder/handler API.
- [ ] Surrounding context describes how `Agent.builder().on_receive_request(...)` and `connect_with(...)` replace the old `impl Agent for ...` pattern.
- [ ] Examples (if any are inline) updated to use builder syntax.

## Tests

- [ ] No code tests — pure doc update. Verify `mdbook` or markdown lint stays clean if applicable.

## Depends on

- Probably best done after all C2-C8 plus AcpServer/AgentTraitImpl reshapes are merged so the doc reflects shipped state.