---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffa80
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

---

## IMPLEMENTATION (complete — in review)

### set_session_mode — claude now emits CurrentModeUpdate
Finding: claude already builds a `CurrentModeUpdate` via `send_mode_update_notification`, but it was buried inside `handle_mode_change_process`, which fires only when `mode_changed == true` AND only after a successful claude-CLI subprocess respawn. llama emits `CurrentModeUpdate` unconditionally after a successful mode set.
Change: hoisted the `send_mode_update_notification` call out of `handle_mode_change_process` into the `set_session_mode` handler (`agent_trait_impl.rs`), called unconditionally after the mode is validated and updated. `handle_mode_change_process` keeps its `if mode_changed` gate — process replacement is an internal claude-specific concern; the client-facing notification is not. claude now emits `CurrentModeUpdate` on every successful `set_session_mode`, identical to llama.

### new_session available-commands — DECISION: neither agent emits an unsolicited AvailableCommandsUpdate
One decision applied to both. Options were (a) both emit, (b) neither emits.
Decision: NEITHER. Rationale: `AvailableCommandsUpdate` is a *change* notification — emitted when the command set changes during a session. claude's `new_session` unconditionally advertised exactly two hard-coded "core" commands (`create_plan`, `research_codebase`) that have no slash-command dispatch handler (referenced only in tests) — non-dispatchable placeholders. llama already emits nothing at `new_session`. claude's genuinely useful MCP-prompt commands are still delivered via `refresh_commands_for_all_sessions` when an MCP server sends `tools/list_changed` / `prompts/list_changed` (agent.rs:593) — independent of the initial emission. Wiring llama's `CommandRegistry` is card 14's scope and was NOT touched.
Change: removed the `send_initial_session_commands` call from claude's `new_session`; deleted the now-orphaned `send_initial_session_commands` method (would otherwise be dead code). `get_available_commands_for_session` / `update_session_available_commands` / `send_available_commands_update` remain — still used by `refresh_commands_for_all_sessions`.

### load_session replay _meta — DECISION: both agents tag replayed notifications
One decision applied to both. Decision: BOTH tag (the task's preferred option — a client should distinguish replayed history from live updates).
Change: added `AcpServer::build_replay_notification` to llama (`acp/server.rs`), tagging every replayed `SessionNotification` with `_meta` identical in shape to claude's `ClaudeAgent::build_replay_notification`: `message_type: "historical_replay"`, `message_index`, `total_messages`. llama's `load_session` replay loop now uses it.

### cancel — DECISION: both agents emit a final status update
One decision applied to both. Decision: BOTH emit. A client cancelling a turn now observes the same cancellation notification from either agent.
Change: added `AcpServer::send_cancellation_update` to llama (`acp/server.rs`), called from `cancel`. It broadcasts an `AgentMessageChunk` carrying `[Session cancelled by client request]`, the text content tagged with `cancelled_at` / `reason` / `session_id` `_meta` and the notification tagged with `final_update` / `cancellation` `_meta` — identical in shape to claude's `send_final_cancellation_updates`. The internal cancellation mechanism (llama request queue vs claude subprocess/tool/permission fan-out) is the essential difference and was intentionally NOT unified.

### Tests
- llama `tests/acp_integration.rs`: added `set_session_mode_emits_current_mode_update`, `cancel_emits_final_status_update`; extended `load_restores_and_replays_history` to assert the `historical_replay` `_meta` tag on each replayed notification.
- `cargo nextest run -p llama-agent`: 1111 passed, 1 skipped.
- `cargo nextest run -p claude-agent`: 309 passed.
- `cargo clippy -p claude-agent -p llama-agent --all-targets`: clean (0 warnings).
- Note: `crates/claude-agent/Cargo.toml` `[lib] test = false` is pre-existing and out of scope; the claude `set_session_mode` change is a structural hoist of an unchanged emission, covered indirectly by claude's integration suite and documented in the handler doc comment.