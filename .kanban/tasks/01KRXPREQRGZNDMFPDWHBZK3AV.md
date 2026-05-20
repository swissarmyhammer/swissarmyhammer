---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffc80
title: Converge ACP initialize, protocol-version, error-mapping, and logging conventions
---
Cross-cutting handler conventions diverge between the two agents. Converge them.

## initialize + protocol version
- claude runs a 3-stage validation chain (`agent_validation.rs`) and treats a protocol-version mismatch as a FATAL error (`handle_fatal_initialization_error`).
- llama does no request-body validation and never fails — `negotiate_protocol_version` always returns a supported version.
- Converge: `initialize` negotiates and does not hard-fail on version mismatch (the spec intent — initialize negotiates). Apply ONE light validation level to both — decide whether claude's deeper validation is kept (and added to llama) or trimmed (and removed from claude); it must match.

## Error mapping
- No shared convention: claude mixes raw `Error::new(-32600/-32602, data)` with named constructors; llama has a `ToJsonRpcError` trait + central `convert_error`, but `ext_method` bypasses it for terminal errors (collapsing the rich `TerminalError` mapping in `error.rs` to a bare `internal_error`).
- `ext_method` unknown-method handling is OPPOSITE: claude returns a SUCCESS `ExtResponse` ("Extension method not implemented"); llama returns `method_not_found()`. Converge — `method_not_found()` is correct.
- Target: one error-construction convention used by both agents (named constructors / a shared `ToJsonRpcError`-style mapping); no raw integer codes scattered in handlers; terminal errors mapped through their real `TerminalError` mapping, not collapsed.

## Logging
- claude has `log_request` / `log_response` on every method; llama is ad-hoc `tracing` per method.
- Converge on one request/response logging convention used by both agents.

## Verify
- The same failure class yields the same error code + shape from both agents.
- Both agents log requests/responses the same way.

---

## DECISIONS MADE (implementation)

### 1. Validation level — TRIM claude to match llama (lightest, identical, non-fatal)
`initialize` on BOTH agents now only negotiates the protocol version and never hard-fails on a version mismatch. Removed claude's `validate_protocol_version`, `handle_fatal_initialization_error`, `perform_initialization_cleanup`, `validate_initialization_request` + helpers, `validate_client_capabilities` + `validate_meta_capabilities`/`validate_filesystem_capabilities`/`validate_terminal_capability`/`get_json_type_name` from the `initialize` path (these were only reachable from `initialize`). `negotiate_protocol_version` + `SUPPORTED_PROTOCOL_VERSIONS` are kept on both. Rationale: the card says a light, non-fatal validation is most spec-aligned and explicitly lists "trim claude's down to match llama's lighter approach" as a valid choice; trimming guarantees genuine consistency with zero cross-crate coupling. (`CapabilityValidator` in `capability_validation.rs` is a separate type used elsewhere and is untouched.)

### 2. Error convention — shared-style `acp_error` helper module per crate
Each crate gets an identical `acp_error` helper module wrapping the ACP named-constructor codes (`invalid_params`, `internal_error`, `invalid_request`, `method_not_found`) with optional custom message + structured `data`. Handler-level raw `Error::new(-32600/-32601/-32602, ...)` literals are replaced with these helpers. llama `ext_method` terminal-operation arms now map `TerminalError` through `Self::convert_error` (the real `ToJsonRpcError` mapping in `error.rs`) instead of collapsing to `internal_error()`. Verified both live `ext_method` paths return `method_not_found()` for unknown methods (claude's success `ExtResponse` was converged to `method_not_found()`).

### 3. Logging — claude's `log_request`/`log_response` discipline applied to both
Added `log_request`/`log_response` helpers to the llama `AcpServer` and called them on every ACP method (`initialize`, `authenticate`, `new_session`, `load_session`, `resume_session`, `list_sessions`, `set_session_mode`, `prompt`, `cancel`, `ext_method`, `ext_notification`), matching claude's convention.

## Review Findings (2026-05-18 23:30)

Reviewed in task-mode. Scope: the ACP-convergence files across both agents (claude-agent: `acp_error.rs`, `agent.rs`, `agent_trait_impl.rs`, `agent_validation.rs`, `agent_file_handlers.rs`, `agent_terminal_handlers.rs`, `agent_prompt_handling.rs`, `content_capability_validator.rs`, `session_resume.rs`, `lib.rs`; llama-agent: `acp/acp_error.rs`, `acp/server.rs`, `acp/content_validation.rs`, `acp/session_resume.rs`, `acp/mod.rs`).

Convergence verdict: the three goals (light non-fatal `initialize`, single error-construction convention, unified request/response logging) are genuinely achieved. The two `acp_error.rs` modules are identical in shape; `log_request`/`log_response` are byte-identical and applied to every ACP method in both agents; `negotiate_protocol_version` is functionally identical; the gutted `agent_validation.rs` removed only dead-after-trim code with no dangling references; llama `ext_method` terminal arms map through the real `ToJsonRpcError` path; both agents return `method_not_found` for unknown ext methods. One consistency finding below.

### Warnings
- [x] `crates/claude-agent/src/agent_trait_impl.rs` / `agent_file_handlers.rs` / `agent_prompt_handling.rs` and `crates/llama-agent/src/acp/server.rs` (`ext_method`) — The `acp_error` helper convention is only half-applied. Many handler error sites still use the bare ACP constructors `Error::invalid_params()` / `Error::internal_error()` (no message) instead of the new `acp_error::*` helpers: in claude `agent_trait_impl.rs` the `validate_fs_read_capability`/`validate_fs_write_capability`/`validate_ext_terminal_capability`/`parse_ext_params`/`to_ext_response` methods use bare constructors while the sibling `validate_editor_capability`/`handle_ext_unknown` in the same file use the helpers; claude `agent_file_handlers.rs` uses the helper for capability checks but bare constructors for IO-error mapping and the `line == 0` check (e.g. line 83, `read_file_with_options`, `apply_line_filtering`, `write_file_atomically`); claude `agent_prompt_handling.rs` uses bare constructors at every error site; llama `server.rs::ext_method` uses bare constructors for all `fs/*` and `terminal/*` capability checks and every `serde_json` parse/serialize error. This is not a correctness bug — the code class is right and no raw integer literals remain — but the same files mix two conventions and lose the descriptive diagnostic messages the helper layer exists to preserve. Converge fully: route handler-local errors through `acp_error::invalid_params(..)` / `acp_error::internal_error(..)` with a descriptive message, so a client gets the same actionable message for the same failure class from either agent.
  - RESOLVED (2026-05-18): Converted every bare-constructor error site in the named ACP handler paths to the `acp_error::*` helpers with descriptive messages. claude `agent_trait_impl.rs` — `validate_fs_read_capability`, `validate_fs_write_capability`, `validate_ext_terminal_capability`, `parse_ext_params`, `to_ext_response`, plus `authenticate` now use `crate::acp_error::*`. claude `agent_file_handlers.rs` — the `line == 0` check, `read_file_with_options` (metadata/size/read IO mapping), `apply_line_filtering` (line-0 + overflow), and `write_file_atomically` (all 7 internal-error sites + the permission-denied invalid-params site) now use `crate::acp_error::*`. claude `agent_prompt_handling.rs` — all 11 bare-constructor sites (content-block processing, turn-request session update, streaming query, streaming text-chunk + tool-call session updates, permission evaluation, image/audio decode, Claude API error, non-streaming chunk-store + assistant-message-store) now use `crate::acp_error::*`. llama `server.rs::ext_method` — top-level params parse, all `fs/*` and `terminal/*` capability checks, every per-arm param parse, session-not-found lookups, and every `serde_json` serialize site now use `super::acp_error::*`. Out of scope and left untouched per the reviewer's explicit file scoping: claude `agent.rs` (not named; mixed non-ACP-handler module) and llama `filesystem_error_to_protocol_error` (a typed-error-enum mapping that already attaches `.data(..)` messages — exactly the category `acp_error.rs`'s module docs route through `ToJsonRpcError` rather than the helpers).

### Nits
- [x] `crates/llama-agent/src/acp/server.rs:1137` vs `crates/claude-agent/src/agent_validation.rs:28` — `negotiate_protocol_version` is functionally identical but cosmetically divergent: claude's is a `pub(crate)` instance method (`&self`), llama's is a private associated function. Harmless, but aligning the signature (both associated fns, or both `&self` methods) would make the "identical convention" claim literally true at the signature level.
  - RESOLVED (2026-05-18): Both `negotiate_protocol_version` functions are now `pub(crate)` associated functions with the identical signature `pub(crate) fn negotiate_protocol_version(client_requested_version: &ProtocolVersion) -> ProtocolVersion`. Dropped `&self` from claude's (it never used instance state) and updated its sole call site to `Self::negotiate_protocol_version(..)`; bumped llama's from private `fn` to `pub(crate) fn`. Both doc comments now note the associated-function convention and the cross-agent alignment.