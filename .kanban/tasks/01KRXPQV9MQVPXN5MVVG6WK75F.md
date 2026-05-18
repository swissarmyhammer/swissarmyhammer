---
assignees:
- claude-code
position_column: todo
position_ordinal: 8c80
title: ACP notification emission parity between claude-agent and llama-agent
---
Both agents should emit the same standard `SessionUpdate` notifications for the same session lifecycle events. Today they emit different subsets.

## Current gaps
- `set_session_mode`: llama emits `SessionUpdate::CurrentModeUpdate` (`acp/server.rs:1092`); claude emits nothing. ACP has this variant precisely for a mode change — a client tracking session mode sees it with llama, not claude.
- `new_session`: claude emits an available-commands update (`update_session_available_commands`); llama emits nothing.
- `load_session` replay: claude tags each replayed `SessionNotification` with `_meta` (`message_type: "historical_replay"`, timestamp); llama sends no `_meta`, so a client cannot distinguish replayed history from live updates.
- `cancel`: claude emits final status updates for pending operations (`send_final_cancellation_updates`); llama emits nothing on cancel, relying solely on the in-flight `prompt` returning `StopReason::Cancelled`. The client-observable notification stream on cancellation differs between the two.

## Target
- claude `set_session_mode` emits `CurrentModeUpdate` like llama.
- One decision for available-commands on `new_session`, applied to both (ties to llama's dead `CommandRegistry` — see the capability advertise/enforce card).
- One decision on replay `_meta`: either both tag historical-replay notifications or neither. If clients should distinguish replay from live updates, both tag identically.
- One decision on cancel-time notifications: either both emit final status updates on cancel or neither. A client cancelling a turn should observe the same thing from both agents. (The internal cancellation fan-out — claude cancels a subprocess + tools + permissions, llama cancels a request queue — is an essential difference and is NOT in scope; only the client-facing notification is.)

## Verify
- A client observes the same notification-stream shape from both agents for: new session, mode change, session-load replay, and cancellation.

Overlaps cards 7 & 9 (load/resume rewire) — coordinate.