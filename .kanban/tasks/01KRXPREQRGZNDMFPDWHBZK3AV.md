---
assignees:
- claude-code
position_column: todo
position_ordinal: '8e80'
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