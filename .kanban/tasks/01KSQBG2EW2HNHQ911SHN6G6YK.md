---
assignees:
- claude-code
depends_on:
- 01KSQBCTMV4K3ATFZ5RFQ0FJBB
position_column: todo
position_ordinal: 8d80
project: llama-coverage
title: Cover ACP message translation (acp/translation.rs) — both directions, pure logic
---
## What

`crates/llama-agent/src/acp/translation.rs` (3k lines) maps between ACP protocol messages and llama-agent's internal types. It is pure data transformation and sits directly on the path between the kanban webview and the agent — a translation bug means the UI shows nothing or shows garbage even when generation is fine. Cover both directions.

## Cover

- **ACP → internal** — `session/prompt` request → internal generation request: content blocks, roles, tool definitions, session id handling.
- **Internal → ACP** — generation chunks/results → ACP `session/update` notifications and the final response: text deltas, tool-call requests, stop/finish reasons, usage/token counts.
- **Content block variants** — text, tool-call, tool-result, image/resource if supported; each round-trips.
- **Error mapping** — internal errors → ACP error responses with correct codes (the `-32603` seen in the bug log; the typed follower/queue errors).
- **Round-trip property** — for representative messages, ACP → internal → ACP preserves semantics.

## Acceptance Criteria

- [ ] Both translation directions covered for every content-block variant the code handles.
- [ ] Error → ACP-error-code mapping pinned.
- [ ] `acp/translation.rs` region coverage reaches the epic threshold (target >95%).
- [ ] No real model and no live transport — pure translation unit tests.

## Tests

- [ ] Unit tests in `acp/translation.rs` `#[cfg(test)]` or `acp/translation/tests.rs`.
- [ ] Run: `cargo test -p llama-agent translation` and confirm the coverage delta.

## Workflow

- Use `/tdd`. Pure logic — independent of the scripted-model harness.