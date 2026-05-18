---
assignees:
- claude-code
depends_on:
- 01KRXHBJ21JY1B71BFHB44BY9W
position_column: todo
position_ordinal: '8480'
title: Add agent-neutral SessionRecord + SessionStore + ResumeStrategy trait to agent-client-protocol-extras
---
Add the agent-neutral session-persistence layer to `agent-client-protocol-extras`. This powers ACP `session/list`, `session/load`, and `session/resume`, independent of which agent backend is in use.

## ACP load vs resume — the distinction this layer encodes
- `session/resume` — restore session state and return. MUST NOT replay history. Gated by `sessionCapabilities.resume`. **This is the primary goal.**
- `session/load` — restore state, then replay the full conversation as `session/update` notifications, then return. Gated by `loadSession`. Must also be supported and tested.

`load` is `resume` + replay. Both share state restoration; that shared structure is what this layer provides.

## Dependency change
`session/resume` types (`ResumeSessionRequest`, `ResumeSessionResponse`, `SessionResumeCapabilities`; method string `"session/resume"`) are behind the upstream `unstable_session_resume` cargo feature in `agent-client-protocol` 0.11 / schema 0.12.0. Enable it on the workspace dependency:
`agent-client-protocol = { version = "0.11", features = ["unstable_session_resume"] }`.
Note: `unstable_*` means the upstream API may shift between releases. (`session/load` types are already stable.) Enabling an upstream crate's optional feature is not the same as adding a workspace feature flag — the workspace "no feature flags" rule does not apply here.

## SessionRecord
Serde-serializable, agent-neutral:
- `session_id` (ULID string), `cwd`, `title: Option<String>`, `updated_at` (RFC3339), `mcp_servers`.
- `updates: Vec<SessionUpdate>` — ordered ACP `SessionUpdate` stream. Replaying it as `session/update` notifications satisfies `session/load`.
- Serialized to `session.json` inside `acp_session_dir(ulid)` (the helper from the RawMessageManager card).

## SessionStore
- `persist(&SessionRecord)` — write `session.json` atomically (temp file + rename).
- `load(ulid) -> Option<SessionRecord>`.
- `list(cwd_filter, cursor) -> (Vec<SessionInfo>, next_cursor)` — scandir `$XDG_STATE_HOME/acp/`, read each `session.json` for the `SessionInfo` fields. ULID directory names sort chronologically → stable cursor pagination for free.

## ResumeStrategy trait — state restoration only
```
trait ResumeStrategy {
    async fn restore(&self, record: &SessionRecord) -> Result<()>;
}
```
Restores the agent's generation state from a record. Per-agent impl in later cards (claude: `claude --resume`; llama: re-render via chat template).
- `session/resume` handler = `restore()` + return.
- `session/load` handler = `restore()` + replay `record.updates` to the client + return.
The replay step is the only difference and lives in shared code.

## Notes
- Replaces claude-agent's `SessionManager` disk persistence and llama-agent's `FileSessionStorage`.
- Unit tests: round-trip persist/load; `list` with/without cwd filter; cursor pagination.

Depends on the shared `RawMessageManager` card (reuses `acp_session_dir`).