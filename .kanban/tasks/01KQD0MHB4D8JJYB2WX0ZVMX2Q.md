---
assignees:
- claude-code
depends_on:
- 01KQD0H1XNTQMW4Z7MRESNJN4X
- 01KQD0H7NHEKP3JEM82F0C43QY
- 01KQD0HAVVP34CDG8HNYVWDEGF
- 01KQD0HE4STQ2K7XJDNC2TN4XJ
- 01KQD0HJ5B0T3X9ATC2Y6TR1TZ
- 01KQD0HN9GCXPJW4364W5XDCGZ
- 01KQD0HVD3AVKHZ0T9YNCWNW5G
- 01KQD0HYXBPSFAPX4MBWDJANPZ
position_column: review
position_ordinal: '80'
project: acp-upgrade
title: 'ACP 0.11: claude-agent: agent_trait_impl.rs → builder/handler reshape'
---
## What

The core of the claude-agent migration: replace the `impl Agent for ClaudeAgent` block in `claude-agent/src/agent_trait_impl.rs` with the new ACP 0.11 builder/handler pattern (`Agent.builder().on_receive_request(...).connect_to(...)`).

Each method on the old trait — `initialize`, `authenticate`, `new_session`, `load_session`, `set_session_mode`, `prompt`, `cancel`, `ext_method`, `ext_notification` — becomes a handler registration on the builder. The handlers can keep delegating to the same internal helpers (`spawn_claude_for_new_session`, `handle_streaming_prompt`, the `handle_ext_*` family, etc.) — only the wiring layer changes.

Files:
- `claude-agent/src/agent_trait_impl.rs`

## Branch state at task start

All claude-agent module fixups landed (B0, B1, B2, B3, B4, B5, B6a, B6b, B6c, B7).

## Acceptance Criteria
- [ ] `agent_trait_impl.rs` compiles. The internal helper methods it calls already work after B2-B7.
- [ ] No remaining `impl Agent for ClaudeAgent` syntax.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests pass.
- [ ] `cargo check -p claude-agent --lib` should now pass for the *library* target (integration tests covered by B10).

## Depends on
- 01KQD0H1XNTQMW4Z7MRESNJN4X (B2).
- 01KQD0H7NHEKP3JEM82F0C43QY (B3).
- 01KQD0HAVVP34CDG8HNYVWDEGF (B4).
- 01KQD0HE4STQ2K7XJDNC2TN4XJ (B5).
- 01KQD0HJ5B0T3X9ATC2Y6TR1TZ (B6a).
- 01KQD0HN9GCXPJW4364W5XDCGZ (B6b).
- 01KQD0HVD3AVKHZ0T9YNCWNW5G (B6c).
- 01KQD0HYXBPSFAPX4MBWDJANPZ (B7).