---
assignees:
- claude-code
depends_on:
- 01KQD0D883ZW5JAA02913DXM8E
position_column: doing
position_ordinal: '8180'
project: acp-upgrade
title: 'ACP 0.11: extras: HookableAgent'
---
## What

Migrate `agent-client-protocol-extras/src/hookable_agent.rs` to ACP 0.11.

`HookableAgent` is the most behavior-rich wrapper — it fans out hook events (SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, PostToolUseFailure, Stop, Notification) to registered handlers and applies their `HookDecision` outputs. Reshape it onto the new builder/handler API; preserve all hook semantics, matchers, and decision propagation.

Files:
- `agent-client-protocol-extras/src/hookable_agent.rs`
- Inline `#[test]` modules in the same file.

Use the pattern established by A1 (`TracingAgent`).

## Branch state at task start

`acp/0.11-rewrite` with `d5b5465bd` + A1's commit.

## Acceptance Criteria
- [ ] `cargo check -p agent-client-protocol-extras --lib` passes for `hookable_agent.rs` (other wrappers may still fail).
- [ ] Public hook event surface (`HookEvent`, `HookDecision`, `HookHandler`, `HookCommandContext`, `SessionSource`, `HookRegistration`) preserved.
- [ ] One commit on `acp/0.11-rewrite`.

## Tests
- [ ] Inline tests in `hookable_agent.rs` pass.
- [ ] Cross-cutting `tests/e2e_hooks/*.rs` are out of scope here (covered by A5).

## Workflow
- Migration guide: https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html

## Depends on
- 01KQD0D883ZW5JAA02913DXM8E (A1: TracingAgent + foundation).