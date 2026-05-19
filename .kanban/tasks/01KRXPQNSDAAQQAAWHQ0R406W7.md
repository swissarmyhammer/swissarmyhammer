---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff980
title: Treat ACP session IDs as opaque strings — remove ULID-format validation, unify not-found errors
---
The two agents handle incoming session IDs inconsistently, and claude-agent over-validates them.

## Principle
A session ID is an **opaque string**. It is valid if and only if the corresponding session exists (its `SessionStore` record / session folder is present) — NOT if it matches a particular format. Do not validate that an incoming ID is a ULID; that check is needless and rejects legitimate IDs. ULIDs are used ONLY when generating a new session ID.

## Current inconsistencies
- claude-agent: `session::SessionId` is a newtype over `Ulid`; `SessionId::parse` calls `Ulid::from_string` and rejects any non-ULID string. `load_session` and `prompt` use it (bad id -> `invalid_params`); `set_session_mode` uses `parse_mode_session_id` (bad id -> `invalid_request` — a different code for the same failure); `cancel` reads the raw string and does not parse at all.
- llama-agent: treats the ID as an opaque string, in-memory map lookup, miss -> `invalid_params`. No format validation — this is the correct shape.

## Target — consistent across both agents
- Remove claude-agent's ULID-format gate on INCOMING IDs. `SessionId::parse` / `Ulid::from_string` must no longer reject non-ULID strings at the protocol boundary. claude-agent may keep a ULID type for GENERATING new IDs, but lookups accept any string.
- Every method that takes a `sessionId` (`session/load`, `session/resume`, `session/prompt`, `session/cancel`, `session/set_mode`) resolves the session the same way: look it up; if absent, return one "session not found" error with one code. Use `invalid_params` (what llama already uses) everywhere; drop claude's `invalid_request` variant. `cancel` resolves the same way (today claude skips parsing).
- One shared resolve-session helper used by both agents.
- This principle also governs `SessionStore` / `acp_session_dir` (the session-record cards): they key on the opaque id string, not a validated ULID.

## Note on claude resume
claude `--resume` derives a UUID from the ULID; that works because claude GENERATES ULIDs. A non-ULID incoming ID simply fails as "not found / not resumable" — a lookup outcome, never an up-front format rejection.

## Verify
- A session is resolvable by its exact stored ID string regardless of format.
- An unknown session ID returns the same error (code + shape) from every method in both agents.

Overlaps cards 6-9 (which touch the same handlers) — coordinate.

## Review Findings 2026-05-18

Task-mode review of the claude-agent opaque-session-id change. The core
requirement is met for the three primary handlers and the fs ext handlers,
but the unification is incomplete for the terminal ext handlers.

### Medium

- [x] **Terminal ext handlers do not use the shared `resolve_session` and
  return a different not-found code.** The task target is "the fs/terminal
  ext handlers" route through one shared resolver and "the not-found error is
  unified to ONE code: `invalid_params` everywhere." The fs handlers
  (`agent_file_handlers.rs:61`, `:172`) were correctly migrated to
  `self.resolve_session(...)` which returns `invalid_params` (-32602). The
  terminal ext handlers were not: `terminal/create`, `terminal/output`,
  `terminal/release`, `terminal/wait_for_exit`, `terminal/kill` route through
  `TerminalManager::validate_session_id` → `resolve_terminal_session`
  (`terminal_manager.rs:478-504`), a *separate* resolver that returns
  `AgentError::Protocol("Session not found: ...")`. `AgentError::Protocol`
  maps to JSON-RPC `-32600` (Invalid Request) per `error.rs:247` — a
  different code from the `-32602` returned everywhere else for the same
  "session not found" failure. A terminal extension call with an unknown
  session id therefore fails with a different error code than `prompt`,
  `cancel`, `set_mode`, and the fs handlers. `resolve_terminal_session` is
  opaque-id-correct (no ULID format gate), so only the error-code unification
  is missing. (Note: card says this overlaps cards 6-9 — coordinate, but the
  divergence is real and in scope for this card's stated target.)

  RESOLVED: `resolve_terminal_session` (`terminal_manager.rs`) now returns
  `AgentError::InvalidRequest` on the session-not-found path instead of
  `AgentError::Protocol`. `InvalidRequest` maps to JSON-RPC `-32602`
  (`invalid_params`) per `error.rs`, so every `terminal/*` handler now fails
  an unknown session id with the same code as `prompt`, `cancel`,
  `set_mode`, and the fs handlers. New integration test
  `opaque_session_ids.rs::terminal_ext_handler_rejects_unknown_session_id_with_invalid_params`
  verifies a `terminal/release` call with both a non-ULID id and an unknown
  ULID returns `InvalidParams`, and that it agrees with `prompt`. Doc
  comments on `resolve_terminal_session` and `get_terminal` updated.

### Low

- [x] **Stale ULID-format validator survives with a misleading comment.**
  `session_validation.rs:147` `validate_session_id` still performs a
  ULID-format gate, and `request_validation.rs:77` `validate_session_id_parameter`
  calls it under a `// Validate ULID format` comment with a
  `session_validation.rs:148` `// ACP requires consistent session ID format
  as raw ULID` comment — directly contradicting the opaque-id principle. This
  is not a correctness violation: `validate_session_id_parameter` is reached
  only from `RequestValidator::validate_load_session_request`, which is
  invoked exclusively from unit tests — the live `load_session` handler calls
  `validate_load_session_mcp_config` (capability gating only) and resolves
  purely via `load_session_record` → `SessionStore`. The format gate at
  `request_validation.rs:91` / `:399` is dead for every live protocol
  boundary. Still worth removing or correcting so the codebase does not carry
  a contradictory "ACP requires ULID format" assertion that a future change
  could wire back onto a live path.

  RESOLVED: `session_validation.rs::validate_session_id` (the ULID-format
  gate) and its four unit tests removed. Both call sites in
  `request_validation.rs` removed: `validate_session_id_parameter` keeps only
  the non-empty check (it is still reachable from `validate_load_session_request`
  for that check), and the `"SessionId"` ULID-format arm in
  `validate_single_parameter_type` was dropped. Contradictory test
  `test_validate_load_session_request_invalid_session_id` replaced with
  `test_validate_load_session_request_accepts_non_ulid_session_id` and
  `test_validate_load_session_request_rejects_empty_session_id`, which assert
  the opaque-id contract. Stale "must be a valid ULID format" comment on
  `TerminalCreateParams::session_id` also corrected. Verified with grep/code
  navigation that `RequestValidator` is referenced only by its own tests.

### Verified clean

- ULID-format gate genuinely removed from the live protocol boundary:
  `resolve_session` (`agent.rs:665`) resolves by existence only — a
  `SessionId::parse` failure is mapped to the same `session_not_found_error`
  as a parse success that misses the cache. `validate_prompt_request` no
  longer format-validates the id.
- Shared `resolve_session` helper used consistently by `session/prompt`
  (`agent_trait_impl.rs:320`), `session/cancel` (`:419`),
  `session/set_mode` (`:278`), `request_permission` (`agent.rs:2304`), and
  the fs ext handlers (`agent_file_handlers.rs:61`, `:172`). `session/load`
  and `session/resume` resolve through `SessionStore` per the same
  resolve-by-existence rule.
- Not-found error unified to `invalid_params` (-32602) for all the above:
  `session_not_found_error` and `restore_error_to_acp` /
  `session_restore_failed_error` all emit -32602. `parse_mode_session_id` is
  fully removed; no `set_session_mode` path returns `invalid_request`.
- `cancel` now resolves the id via `resolve_session` before any cancellation
  work, instead of reading the raw string.
- Resume safety holds: `ResumeStrategy::restore` (`session_resume.rs:421`)
  calls `SessionId::parse(&record.session_id)` and maps any failure to
  `SessionRestoreError::UnusableId` before `to_uuid_string` is ever reached.
  `to_uuid_string` only runs on a real `SessionId(Ulid)`; a non-ULID id fails
  gracefully as a resume miss, never a panic.
- `SessionStore` / `acp_session_dir` keying is path-safety-only:
  `session_path_component` (`raw_messages.rs:91`) rejects empty / `.` / `..` /
  path separators and accepts any other non-ULID single-component id.
- New `tests/integration/opaque_session_ids.rs` genuinely verifies the
  unified behavior: a non-ULID id and an unknown ULID both yield
  `InvalidParams` for `prompt`, `cancel`, and `set_session_mode`, and all
  three handlers are asserted to share one code. Module is wired into
  `tests/integration/mod.rs`.