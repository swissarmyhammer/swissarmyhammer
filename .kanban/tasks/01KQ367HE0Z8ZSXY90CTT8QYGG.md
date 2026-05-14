---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff780
project: acp-upgrade
title: 'Spike: bump agent-client-protocol to 0.11 and capture compile errors'
---
## What

Research spike to discover the *actual* breaking surface from 0.10.4 → 0.11.1 in this workspace.

## Acceptance Criteria
- [x] `Cargo.toml` workspace dep `agent-client-protocol` bumped to `"0.11"` (saved on scratch branch `spike/acp-0.11`, commit `f206917c8`).
- [x] `cargo check --workspace --all-targets` output captured in findings below.
- [x] Per-crate tasks in this project updated with the concrete error list and classification.
- [x] Stale `fix_tests_for_acp_0_9_0` feature investigated; cleanup captured below.

## Tests
- [x] No new tests in this task — research only.

---

# Spike Findings

## Headline result

**ACP 0.11.0 is a complete SDK redesign, not a SemVer-bump.** The official changelog lists it as "Migrate to new SDK design" (PR with a dedicated migration guide at https://agentclientprotocol.github.io/rust-sdk/migration_v0.11.x.html . Every `Agent` impl in the workspace must be **rewritten**, not patched.

`cargo check --workspace --all-targets` (with `--keep-going`) failed early on `agent-client-protocol-extras` (23 errors) and the rest of the dependency tree could not be checked because every other ACP-consuming crate depends on `agent-client-protocol-extras`. The spike therefore enumerated each crate's ACP usage by source-grep and cross-referenced against the new 0.11.1 module layout to project the per-crate breakage.

## Architectural changes

1. **`Agent` is no longer a trait.** In 0.11 it is a unit struct (`agent_client_protocol::Agent`, defined in `role::acp::Agent`) used as a Role marker. The old `#[async_trait::async_trait(?Send)] impl Agent for MyAgent { async fn initialize(...) ... }` pattern does not exist. Agents are constructed via a builder + handler pattern: `Agent.builder().on_receive_request(...).on_receive_dispatch(...).connect_to(stream)`. See `examples/simple_agent.rs` in the 0.11.1 source.
2. **Schema types moved into the `schema` submodule.** `InitializeRequest`, `ContentBlock`, `SessionUpdate`, `SessionNotification`, `ToolCall`, `StopReason`, etc. now live at `agent_client_protocol::schema::*`. Only `Error`, `ErrorCode`, `Result`, the role markers (`Agent`, `Client`, `Conductor`, `Proxy`), the six message-enum types (`AgentRequest`, `AgentResponse`, `AgentNotification`, `ClientRequest`, `ClientResponse`, `ClientNotification`), and the JSON-RPC plumbing types are re-exported at the crate root.
3. **`SessionNotification` still exists** as a struct (in `agent-client-protocol-schema-0.12.0`), but the cargo error message "no `SessionNotification` in the root" is misleading — it just moved to `schema::`. This applies to ~30 other types referenced across the workspace.
4. **`Error::auth_methods` was removed in 0.11.0** (changelog 0.11.0). Audit error-construction sites in `agent-client-protocol-extras/src/hook_config.rs`.
5. **Schema crate jumped 0.11.4 → 0.12.0** (and 0.12.2 is available). All schema types are still `#[non_exhaustive]` (carried over from schema 0.8.0) so existing match arms must continue to handle new variants — but in practice every match arm has to be re-written anyway because of the type-path change.
6. **New transitive deps pulled in by 0.11.1**: `agent-client-protocol-derive 0.11.0`, `futures-concurrency 7.7.1`, `futures-lite 2.6.1`, `jsonrpcmsg 0.1.2`, `pin-project 1.1.11`. `async-broadcast` is no longer a transitive dep.

## Per-crate breakage projection

Counts are unique `agent_client_protocol::*` references found by grep on the source tree (after the version bump applied on `spike/acp-0.11`).

| Crate | Lines referencing `agent_client_protocol` | Has `Agent` trait impl? | Adaptation cost |
|---|---:|---|---|
| `agent-client-protocol-extras` | 225 | Yes (3 impls: HookableAgent, TracingAgent, RecordingAgent — wrappers around `Arc<dyn Agent + Send + Sync>`) | **Rewrite** — entire wrapper-agent abstraction depends on the old trait |
| `claude-agent` | 754 | Yes (`agent_trait_impl.rs` is a full `impl Agent`) | **Rewrite** — largest surface; full agent on the old trait |
| `llama-agent` | 504 | Yes (`acp/server.rs` is a full `impl Agent`) | **Rewrite** — second-largest surface; AcpServer on the old trait |
| `acp-conformance` | 188 | Multiple mock `Agent` impls in src + tests | Rewrite of mocks; fixture wire-format must be re-validated |
| `swissarmyhammer-agent` | 47 | Uses `dyn Agent` and `TracingAgent` | Rewrite of the dyn-Agent wrapper; mostly downstream of extras |
| `swissarmyhammer-tools` | 0 (Cargo.toml dep only) | No | **Drop the dep** — it's unused (see Cleanup below) |
| `avp-common` | 8 | Uses `Agent` and `SessionNotification` (test helpers + executor) | Small adaptation; mostly downstream of extras |

### `agent-client-protocol-extras`

23 compile errors observed. They split into two categories that the per-crate task must handle:

**(a) Trivial path/type rename** (mechanical fix is `agent_client_protocol::X` → `agent_client_protocol::schema::X`): all 22 schema types in the `use` blocks of `hookable_agent.rs`, `tracing_agent.rs`, `recording.rs`, `playback.rs`, `hook_config.rs` (`AuthenticateRequest`, `AuthenticateResponse`, `CancelNotification`, `ContentBlock`, `ExtNotification`, `ExtRequest`, `ExtResponse`, `InitializeRequest`, `InitializeResponse`, `LoadSessionRequest`, `LoadSessionResponse`, `NewSessionRequest`, `NewSessionResponse`, `PromptRequest`, `PromptResponse`, `SessionNotification`, `SessionUpdate`, `SetSessionModeRequest`, `SetSessionModeResponse`, `StopReason`, `TextContent`, `ToolCallStatus`).

**(e) Architectural rewrite** (NOT a trivial signature change — the abstraction is gone): all 6 sites that say `impl Agent for ...` or `Arc<dyn Agent + Send + Sync>` (in `lib.rs`, `hookable_agent.rs`, `tracing_agent.rs`, `playback.rs`, `recording.rs`, `hook_config.rs`). The `AgentWithFixture: Agent` supertrait bound is also broken — the whole trait-bound design needs to be replaced by a wrapper that adapts the new builder-style agent surface to the wrapping/recording/playback usecase.

### `claude-agent`

Cannot be type-checked while extras is broken, but by inspection of `agent_trait_impl.rs`, every method (`initialize`, `authenticate`, `new_session`, `load_session`, `set_session_mode`, `prompt`, `cancel`, `ext_method`, `ext_notification`) is on the old trait shape. **The whole file is a rewrite.** Same for the 728 inline `#[test]` / `#[tokio::test]` cases inside `claude-agent/src/**/*.rs` (currently all gated off by `lib.test = false`). The `protocol_translator.rs` ContentBlock matching, the session/plan/tools/terminal/editor modules, and the integration-test mocks all touch the renamed schema types.

`CollectedResponse` referenced in a comment in `claude-agent/src/lib.rs:96` (as `agent_client_protocol::CollectedResponse`) — that type does not exist in 0.11. Stale comment, drop the reference.

### `llama-agent`

Same architectural rewrite story as claude-agent, slightly smaller surface. Heavy reliance on `match` over `SessionUpdate`, `ContentBlock`, `ContentChunk`, `ToolKind`, `ToolCallStatus` in `acp/translation.rs`. All 9 `Agent` trait methods on `AcpServer` need re-housing into a builder/handler graph.

### `acp-conformance`

Each scenario file under `src/` defines a mock `Agent`. All of those rewrite. Fixtures under `.fixtures/llama` and `.fixtures/claude` should still deserialize cleanly because `SessionNotification`, `SessionUpdate`, etc. are unchanged at the wire-format level (no schema-level changes between schema 0.11.4 and 0.12.0 that I observed, but a full fixture replay is required to confirm).

### `swissarmyhammer-agent`

Imports `Agent`, `TracingAgent`, plus a handful of schema types. Most of the breakage is downstream of the extras crate — once extras compiles in the new style, swissarmyhammer-agent's adaptation is small (rewrite the `TracingAgent` consumption to whatever shape it gains).

### `swissarmyhammer-tools` — drop the dep

`grep -r "agent_client_protocol" swissarmyhammer-tools/src/` returns **zero** matches. The only mention is `agent-client-protocol = { workspace = true }` in `Cargo.toml`. This dep is dead. The per-crate task should **delete the line from `swissarmyhammer-tools/Cargo.toml`** and not touch any source.

### `avp-common`

Uses `Agent` (in `context.rs`, `validator/runner.rs`), `SessionNotification` (same files), and `StopReason` (in `validator/executor.rs`). Plus `agent_client_protocol_extras::PlaybackAgent` in tests. After extras is rewritten to expose a new wrapper type, avp-common's edits are: change `Agent` consumption shape, switch `SessionNotification`/`StopReason` import paths to `schema::`.

## Cleanup: `lib.test = false` and `fix_tests_for_acp_0_9_0`

- `claude-agent/Cargo.toml` line 12: `test = false  # Disable lib cfg(test) modules - need ACP 0.9.0 fixes` — references **ACP 0.9.0** (current dep is 0.10.4, target is 0.11.1). The justification is two major ACP versions stale.
- `claude-agent/Cargo.toml` lines 51–53: `[features] fix_tests_for_acp_0_9_0 = []` — declared feature is **never referenced** anywhere in the workspace (`grep -r "fix_tests_for_acp_0_9_0" --include="*.rs"` returns zero matches). It's dead config.
- `claude-agent/src/**/*.rs` contains **728 inline `#[test]` / `#[tokio::test]` cases** that are all currently disabled by the `lib.test = false`. That's a large coverage hole.

**Recommendation**: the claude-agent adaptation task (01KQ3699DCCSTFXHK772WQKVMA) should drop both `[lib] test = false` and `[features] fix_tests_for_acp_0_9_0 = []`, and re-enable the 728 inline tests. Whatever was originally broken at the ACP 0.9.0 transition is going to be rewritten anyway during the 0.11 migration, and the inline tests are the best way to validate the rewrite.

## Workflow

- Spike commit lives on `spike/acp-0.11` (commit `f206917c8`). The commit only touches `Cargo.toml` and `Cargo.lock`. The branch was not merged or pushed.
- The current `mcp` branch is back at `agent-client-protocol = "0.10"` with `Cargo.lock` unchanged.
- Per-crate task descriptions have been updated to reflect the rewrite scale (not "adaptation"). Re-classify the ACP-upgrade project as a **rewrite project**, not a refactor — re-estimate the timebox accordingly.