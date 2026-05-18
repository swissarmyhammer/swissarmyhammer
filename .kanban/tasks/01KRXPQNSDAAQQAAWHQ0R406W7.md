---
assignees:
- claude-code
position_column: todo
position_ordinal: 8b80
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