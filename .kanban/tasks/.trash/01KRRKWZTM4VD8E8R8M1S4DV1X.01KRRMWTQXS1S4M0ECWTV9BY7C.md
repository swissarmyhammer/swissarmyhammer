---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8680'
project: ai-panel
title: Prompt execution + session/update notification translation to Tauri events
---
## What
Make a session usable: send a prompt and stream the agent's reply back to the webview.

- In `apps/kanban-app/src/ai/session.rs`: add `prompt(text)` — issues the ACP `prompt` request on the `ConnectionTo<Agent>`. The `PromptResponse` carries only the `StopReason`; the content arrives via `session/update` notifications.
- In `apps/kanban-app/src/ai/client.rs` (or a new `ai/translation.rs`): implement the `session/update` notification handler. Translate each `SessionUpdate` variant to a `UIMessageChunk`-shaped payload and emit it as a Tauri event keyed by window:
  - `AgentMessageChunk` -> text part
  - `AgentThoughtChunk` -> reasoning part
  - `ToolCall` / `ToolCallUpdate` -> tool part (name, args, status, result)
  - `AgentPlan` -> plan/task part
  - `AvailableCommandsChanged` -> available-commands part
- Emit `ai://chunk/{window_label}` for content parts and `ai://status/{window_label}` for `{ state: idle|streaming|error, stopReason?, error? }` on completion.
- Notifications must arrive through the registered ACP `Client` notification handler — never a broadcast side channel.

Spec: `ideas/kanban/ai_panel.md` — Phase 2 "Notification handling", and the Wire Protocol section.

## Acceptance Criteria
- [ ] `prompt(text)` issues the ACP `prompt` request and returns when the agent stops.
- [ ] Each `SessionUpdate` variant is translated to the documented `UIMessageChunk` part shape.
- [ ] Content parts are emitted as `ai://chunk/{window_label}`; completion as `ai://status/{window_label}`.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Unit tests: each `SessionUpdate` variant -> expected `UIMessageChunk` part (one test per variant: message, thought, tool call, plan, commands-changed).
- [ ] Integration test: prompt against an in-process test ACP Agent that emits known `session/update` notifications; capture emitted Tauri events and assert the chunk sequence and the terminal `ai://status` event.
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the per-variant translation tests first, then the prompt integration test.