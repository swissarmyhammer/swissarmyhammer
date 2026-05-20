---
assignees:
- claude-code
depends_on:
- 01KRXHBJ21JY1B71BFHB44BY9W
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff080
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

## Review Findings (2026-05-18 17:02)

### Warnings
- [x] `crates/agent-client-protocol-extras/src/session_store.rs:178` — `persist` temp file is named `session.json.<pid>.tmp`, but the doc comment claims this prevents concurrent persists from clobbering each other's temp file. The PID is identical for every thread in one process, so two concurrent `persist` calls for the same session race on the *same* temp path — the two `std::fs::write` calls can interleave and corrupt the temp content before either rename. The atomic rename still protects the final `session.json`, but the claim in the comment is false. Fix: make the temp name unique per call (append a thread id, an atomic counter, or a random suffix), or correct the comment to scope the guarantee to cross-process only. RESOLVED: temp name now includes a process-wide `AtomicU64` counter (`session.json.<pid>.<counter>.tmp`), making every `persist` write a distinct temp file regardless of thread; doc comment updated to match.
- [x] `crates/agent-client-protocol-extras/src/raw_messages.rs:54` — `acp_session_dir` joins the session id straight into a path (`base.join("acp").join(session_ulid)`) with no path-component sanitization. `SessionStore::load(session_id)` is the first client-facing read path to feed an externally-influenced id through this helper. A session id containing `/` or `..` would resolve outside `acp/`. Server-generated ULIDs make this non-exploitable today, but `load` widens the exposure. Recommend rejecting ids that are not a single non-`.`/`..` path component before the join (this preserves the opaque-string contract — it validates path safety, not ULID format). RESOLVED: added `session_path_component`, which rejects empty, `.`, `..`, and ids containing `/` or `\` with `ErrorKind::InvalidInput`; `acp_session_dir` validates before joining. The single helper is shared by `raw_messages.rs` and (via re-use of `acp_session_dir`) `session_store.rs`. New tests `test_acp_session_dir_rejects_path_escaping_ids` and `test_acp_session_dir_accepts_safe_non_ulid_id`.

### Nits
- [x] `crates/agent-client-protocol-extras/src/session_store.rs:316` — `sessions_cursor` / the `page_size == 0` interaction: a `list(.., 0)` call against a store that has sessions breaks the loop on the first iteration with an empty `sessions` vec and returns `next_cursor: Some("")` (empty-string cursor). Following that cursor happens to behave (empty page), but emitting a `Some("")` cursor for a deliberately-empty page is sloppy. The `list_with_zero_page_size_is_empty` test only asserts `sessions.is_empty()` and never inspects `next_cursor`, so this is untested. Suggest returning `next_cursor: None` when `page_size == 0` (or asserting the intended cursor value in that test). RESOLVED: `sessions_cursor` now returns `Option<String>` (`None` for an empty page), so `page_size == 0` yields `next_cursor: None`; `list_with_zero_page_size_is_empty` now asserts `next_cursor.is_none()`.
- [x] `crates/agent-client-protocol-extras/src/session_store.rs:224` — No test covers `next_cursor` resuming correctly across a *filtered* page boundary, nor the case where a `cwd_filter` leaves trailing sessions all filtered out (which yields a `next_cursor` pointing at an effectively-empty next page). Pagination is correct, but a filtered-pagination test would lock in the cursor-vs-filter interaction. RESOLVED: added `list_paginates_with_cursor_and_cwd_filter` (cursor advances across interleaved non-matching sessions) and `list_cursor_into_all_filtered_tail_is_empty` (cursor lands in an all-filtered tail → empty page, no cursor).