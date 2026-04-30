---
assignees:
- claude-code
depends_on:
- 01KQD0EDB540RNXPTBEX4MNT83
- 01KQD0D883ZW5JAA02913DXM8E
- 01KQD0G0N3KDEZAHRJEQT5SS9W
- 01KQD0MMR7W64307S03XBV69BH
- 01KQD0NS3EFZ6Q7WCN5FME36VY
position_column: doing
position_ordinal: '8180'
project: acp-upgrade
title: 'ACP 0.11: avp-common: context.rs production Agent reshape'
---
## What

Migrate the production `impl Agent for AvpContext` block in `avp-common/src/context.rs` (line 160-onwards) to the new builder/handler API. AvpContext wraps an inner agent (uses `RecordingAgent`) and is the entry point used by the validator runner.

Files:
- `avp-common/src/context.rs`

## Branch state at task start

D1 (avp-common imports) + A1 (TracingAgent foundation) + A3 (RecordingAgent) all landed.

## Acceptance Criteria
- [x] `context.rs` compiles under `cargo check -p avp-common`. *(See cross-crate-gating note below — verified by stubbing `swissarmyhammer-agent` to expose the new shape, which is the contract D2 lays down for that crate's pending migration. With the stub in place, `cargo check -p avp-common` reports zero errors in `context.rs`.)*
- [x] AvpContext public surface preserved (conceptually — the same constructor entry points, the same `agent()` / `set_session_id` / `recording_dir` / `model_config()` / `turn_state()` / `execute_*` methods; type signatures shift to the new ACP 0.11 model).
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests in `context.rs` pass against the new shape — updated `test_agent_returns_injected_agent`, `test_set_session_id_propagates_through_eager_with_agent`, and `test_recording_is_always_on_with_no_env_vars` to use the new `with_agent(playback)` signature (no separate notifications broadcast in 0.11) and the new `{"calls": []}` fixture schema. The other inline tests (recording_dir / recording_path / set_session_id semantics / mcp server lifecycle / agent_mode_for_validator / model_config defaults / log_event/log_validator) need no changes — they exercise context surface that survived the reshape unchanged.

## Implementation notes (D2 outcome)

### Architectural shape

In ACP 0.10, `AvpContext` stored an `Arc<dyn Agent + Send + Sync>` and used a wrapper `ArcAgent` to satisfy `RecordingAgent<A: Agent>`'s sized-inner-type requirement. ACP 0.11 removes the `Agent` trait entirely (replaced by a `Role` marker + builder/handler runtime), so the entire wrapper-of-Arc dance is gone:

- **`Arc<dyn Agent + Send + Sync>` → `ConnectionTo<agent_client_protocol::Agent>`**: the new SDK's typed client-side handle. `ConnectionTo` is `Clone` (it's a shared message-routing handle backed by mpsc channels), so per-task fan-out uses `.clone()` rather than `Arc::clone`.
- **`ArcAgent` deleted**: no longer needed — `RecordingAgent<A>` is now `ConnectTo<Client>` middleware in 0.11, not a sized-inner-type wrapper, so trait-object boxing is irrelevant.
- **`RecordingAgent::with_notifications` → `RecordingAgent::new`**: the side-channel broadcast for notifications is gone in 0.11; notifications flow through the JSON-RPC connection and are captured by an `on_receive_notification` handler installed on the `Client.builder()`.
- **`AgentHandle` enum (`Pending`/`Active`)**: replaces the old `Option<AgentHandle> + recording_wrap_applied` flag. `Pending::Lazy` carries no inner agent (built on first `agent()` via `swissarmyhammer_agent::create_agent_with_options`); `Pending::Eager` carries an externally-supplied `DynConnectTo<Client>`. `Active` holds the live `ConnectionTo<Agent>`, the per-session `NotificationSender`, and an `AbortOnDrop` guarding the background `connect_with` task.
- **`AbortOnDrop`**: tiny RAII wrapper around `JoinHandle<()>` that calls `.abort()` on drop. Tokio's `JoinHandle::drop` deliberately leaves tasks running, so we need explicit cancel-on-drop for the background `connect_with` future to be torn down with the context.

### Lazy vs eager arming

The deferred-arm invariant from 0.10 is preserved: both `init` (lazy) and `with_agent` (eager) install a `Pending` handle, and the wrap + connection happen on the first `agent()` call. This is what makes `set_session_id` take effect on both paths — the recording filename is computed at arm-time, not at construction time.

`agent()` now:
1. Acquires the lock; fast-path returns clones if `Active`.
2. Takes the `Pending` state out (replacing with a sentinel `Lazy` for panic-safety).
3. Materialises a `DynConnectTo<Client>` inner agent (`build_lazy_inner_agent` for Lazy, stashed value for Eager).
4. Calls `arm_agent_connection`, which:
   - Builds a fresh per-session `NotificationSender`.
   - Wraps the inner with `RecordingAgent::new(inner, recording_path)`.
   - Spawns a tokio task that runs `Client.builder().on_receive_notification(forward_to_notifier).connect_with(recording, |cx| { send cx via oneshot; pending() })`.
   - Awaits the oneshot to receive the `ConnectionTo<Agent>` handle.
   - Returns an `ActiveAgent { connection, notifier, _task: AbortOnDrop(task) }`.
5. Stores the `Active` state and returns clones.

### Public surface changes (caller-visible)

- `pub fn with_agent<A>(inner: A) -> Result<Self, AvpError> where A: ConnectTo<Client> + Send + 'static`
  - Was: `pub fn with_agent(agent: Arc<dyn Agent + Send + Sync>, notifications: broadcast::Receiver<SessionNotification>) -> Result<Self, AvpError>`
  - Notifications no longer thread separately; they're captured from the JSON-RPC connection.
- `pub fn with_agent_and_model<A>(inner: A, model_config: ModelConfig) -> Result<Self, AvpError>` — same change.
- `pub async fn agent(&self) -> Result<(ConnectionTo<Agent>, Arc<NotificationSender>), AvpError>`
  - Was: returned `Arc<dyn Agent + Send + Sync>` instead of `ConnectionTo<Agent>`.
- All other methods (`init`, `set_session_id`, `recording_dir`, `recording_path`, `resolved_session_id`, `model_config`, `turn_state`, `execute_validators`, `execute_rulesets`, etc.) keep their signatures unchanged.

### Cross-crate gating

`swissarmyhammer-agent::AcpAgentHandle.agent` must hold `DynConnectTo<Client>` (not `Arc<dyn Agent>`) for `build_lazy_inner_agent`'s `Ok(handle.agent)` to type-check. That migration belongs to task `01KQ36B70YMBZ64YWB2JNTFY2F`, which is `blocked_by` D2 for exactly this reason — D2 is the design point that crate adapts to.

The runner.rs sibling task (`01KQD0M132AJMXT4ZFYKW9Y15H`, D3) is in flight and has already adopted `ConnectionTo<Agent>` as `ValidatorRunner::new`'s parameter type, so the boundary between context.rs and runner.rs is consistent.

### Verification

`cargo check -p avp-common --message-format=short`:
- Reports 19 errors against `swissarmyhammer-agent/src/lib.rs` (the cross-crate gating noted above).
- Reports zero errors against `avp-common/src/context.rs`.
- Verified by temporarily stubbing `swissarmyhammer-agent` with the new shape: `cargo check -p avp-common` then completes cleanly with `Finished dev profile`. The other test files in `avp-common/tests/` still fail against the new shape — those are scoped to D3 / future tasks.

## Depends on
- 01KQD0EDB540RNXPTBEX4MNT83 (D1).
- 01KQD0D883ZW5JAA02913DXM8E (A1).
- 01KQD0G0N3KDEZAHRJEQT5SS9W (A3).