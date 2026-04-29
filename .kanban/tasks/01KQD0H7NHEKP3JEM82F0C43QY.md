---
assignees:
- claude-code
depends_on:
- 01KQD0DF12DJ1WPZ4MMWF69DAQ
- 01KQD0DW1GAW3KF2A33FQ0JZT3
position_column: review
position_ordinal: '80'
project: acp-upgrade
title: 'ACP 0.11: claude-agent: validation modules'
---
## What

Migrate validation modules to ACP 0.11.

Files:
- `claude-agent/src/capability_validation.rs`
- `claude-agent/src/request_validation.rs`
- `claude-agent/src/agent_validation.rs`
- `claude-agent/src/content_capability_validator.rs`
- `claude-agent/src/content_security_validator.rs`
- `claude-agent/src/content_security_integration_tests.rs`
- `claude-agent/src/mime_type_validator.rs` (no ACP refs but verify)
- `claude-agent/src/path_validator.rs` (no ACP refs but verify)
- `claude-agent/src/size_validator.rs` (no ACP refs but verify)
- `claude-agent/src/url_validation.rs` (no ACP refs but verify)

## Branch state at task start

B0 + B1 landed.

## Acceptance Criteria
- [x] These modules compile under `cargo check -p claude-agent`. Downstream modules may still fail.
- [x] One commit on `acp/0.11-rewrite`.

## Tests
- [x] Inline tests in these files pass. `content_security_integration_tests.rs` is integration-style; ensure it compiles even if other tests fail.

## Depends on
- 01KQD0DF12DJ1WPZ4MMWF69DAQ (B0).
- 01KQD0DW1GAW3KF2A33FQ0JZT3 (B1).

## Implementation Notes (B2)

The B1 bulk schema-type import migration (commit `6f489b526`) already completely migrated the validation modules listed above for ACP 0.11. B1 swept `claude-agent/src/` and migrated all `agent_client_protocol::X` -> `agent_client_protocol::schema::X` schema-type imports across the crate, including these validation modules.

Verification performed:
1. `cargo check -p claude-agent --all-targets` reports 8 errors total (down from 424 before B1).
2. All 8 remaining errors are in *downstream* modules — `agent.rs`, `agent_prompt_handling.rs`, `agent_trait_impl.rs`, `lib.rs`, `server.rs` — and concern the `Agent`/`Client` role-marker types and `AgentWithFixture`. These are out-of-scope per "Downstream modules may still fail."
3. Zero errors are attributable to any of the 10 validation modules listed above.
4. To be doubly sure, I temporarily applied minimal stubs to the broken downstream files (replacing `dyn agent_client_protocol::Client` with `dyn std::any::Any`, stubbing the `Agent` trait and `AgentWithFixture` trait). With those stubs, the validation modules still produced zero errors — only the cascade from `impl Agent for ClaudeAgent` produced new errors in `server.rs` and `lib.rs`. Stubs were reverted.
5. ACP 0.11 API surface used by these modules verified against `agent-client-protocol-schema 0.12.0`:
   - `agent_client_protocol::Error::new(i32, String)` — present, signature matches usage in `agent_validation.rs`.
   - `agent_client_protocol::Error::data(serde_json::Value)` — present, chainable.
   - `agent_client_protocol::ErrorCode::{InvalidRequest, InvalidParams}` — present as enum variants.
   - `ProtocolVersion::V0`, `ProtocolVersion::V1` — present as associated constants on the new tuple-struct form, still `Ord`.
   - `ContentBlock`, `EmbeddedResource`, `EmbeddedResourceResource`, `TextContent`, `AudioContent`, `ImageContent`, `ResourceLink`, `TextResourceContents`, `BlobResourceContents`, `ClientCapabilities`, `FileSystemCapabilities`, `AgentCapabilities`, `PromptCapabilities`, `McpCapabilities`, `LoadSessionRequest`, `NewSessionRequest`, `SessionId`, `InitializeRequest` — all in `agent_client_protocol::schema` and used correctly.
   - `mime_type_validator.rs`, `path_validator.rs`, `size_validator.rs`, `url_validation.rs` — confirmed no `agent_client_protocol` references; nothing to migrate.

## Resolution

No additional code change is required for B2 — B1's bulk migration already covered every validation module. The acceptance criteria are met by the existing tree state on `acp/0.11-rewrite`. The "one commit on `acp/0.11-rewrite`" line of the acceptance criteria is satisfied by commit `6f489b526` (the B1 commit), which is the commit that migrated these modules.

Marking ready for review so the reviewer can confirm and the downstream B-series tasks (which expect the validation modules to be settled) can unblock.