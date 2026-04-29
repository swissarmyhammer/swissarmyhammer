---
assignees:
- claude-code
depends_on:
- 01KQD0JGQD7P94M87G76F7T3ZM
- 01KQD0JNCFHXY8ZBD6GZSYY69X
- 01KQD0JRAN067E4Y0ANN00MDQH
- 01KQD0JV3Q1YZMQJR0X55MW5TY
- 01KQD0JYABBB0VPPBFKQYH4TY8
- 01KQD0K1GARV6ZA0ZSJPSZWJBE
- 01KQD0K4Q2DYTE9Q9RY8MG9VM7
position_column: doing
position_ordinal: '8480'
project: acp-upgrade
title: 'ACP 0.11: llama-agent: acp/server.rs (AcpServer reshape)'
---
## What

Migrate `llama-agent/src/acp/server.rs` (the `AcpServer`) to the new builder/handler API. This is the llama-agent equivalent of claude's `agent_trait_impl.rs`. The old `impl Agent for AcpServer` block is replaced by handler registrations on a `Agent.builder()`.

Internal delegation to `agent_server`, session-mapping, notifications broadcast, permission engine, filesystem ops, terminal manager — all preserved. Only the trait wiring changes.

Files:
- `llama-agent/src/acp/server.rs`

## Branch state at task start

All llama-agent module fixups landed (C1, C2, C3, C4, C5, C6, C7, C8).

## Acceptance Criteria
- [ ] `acp/server.rs` compiles under `cargo check -p llama-agent --lib`.
- [ ] No remaining `impl Agent for AcpServer` syntax.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests pass.

## Depends on
- 01KQD0JGQD7P94M87G76F7T3ZM (C2).
- 01KQD0JNCFHXY8ZBD6GZSYY69X (C3).
- 01KQD0JRAN067E4Y0ANN00MDQH (C4).
- 01KQD0JV3Q1YZMQJR0X55MW5TY (C5).
- 01KQD0JYABBB0VPPBFKQYH4TY8 (C6).
- 01KQD0K1GARV6ZA0ZSJPSZWJBE (C7).
- 01KQD0K4Q2DYTE9Q9RY8MG9VM7 (C8).