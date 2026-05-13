---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffc280
project: rebuild-index
title: Typed follower error for write ops
---
When a non-leader process attempts a write op (today: `build status` / soon: `rebuild index`, `clear status`), SQLite returns "attempt to write a readonly database" and we surface it as opaque `-32603: database error`. This is confusing because the user has no idea why — the actual cause is that another process (typically an MCP server running for an agent session in this repo) holds the leader lock and the writable connection.

## Design

Add a typed error to `swissarmyhammer-code-context/src/error.rs`:

```rust
#[error("rebuild index needs the writable database, but this process opened it read-only. \
The code-context leader (pid {leader_pid}, db {db_path}) is currently holding the writer — \
usually an MCP server running for an agent session in {workspace_root}. \
Stop that session and rerun, or invoke this op through it.")]
ReadOnlyFollower {
    leader_pid: Option<u32>,
    workspace_root: PathBuf,
    db_path: PathBuf,
},
```

`leader_pid` is read from the flock file used by `swissarmyhammer-leader-election` — that crate already keeps the leader's pid for the lock; expose a `peek_leader_pid(&Path)` if not already public. If we can't read it, fall back to `None` and word the message accordingly.

## Where to detect

The `Workspace::db()` path returns a `DbRef::Owned(&Connection)` for followers. The cleanest fix: add a `Workspace::write_db()` that returns `Result<DbRef<'_>, CodeContextError>`, where the follower branch returns `Err(ReadOnlyFollower { ... })`. Then update `rebuild_index` and `clear_status` ops to call `write_db()` instead of `db()`. Read ops keep calling `db()`.

This way the error fires *before* we attempt the SQL, with a clear message, rather than relying on string-matching SQLite errors after the fact.

## MCP surface

In `mod.rs::context_err` (or wherever we map `CodeContextError` to `McpError`), map `ReadOnlyFollower` to `McpError::invalid_request` with the same message body. The user sees the explanation in the MCP error payload instead of `-32603: database error`.

## Tests

- Unit: `Workspace::write_db()` on a follower workspace returns `ReadOnlyFollower` with the right pid/path
- Integration: spawn a leader workspace, open a second workspace as follower, attempt `rebuild_index`, assert error variant
- MCP-level: existing test pattern in `mod.rs` tests — call `rebuild index` against a follower-only workspace, assert error message contains the workspace path

#code-context #error-handling #rebuild-index

## Review Findings (2026-05-13 13:25)

### Warnings
- [x] `swissarmyhammer-code-context/src/error.rs:44` — The `ReadOnlyFollower` Display message hard-codes the op name: `"rebuild index needs the writable database, but this process opened it read-only."` But the same error variant is now returned from `write_db()` for *both* `execute_rebuild_index` (mod.rs:2283) and `execute_clear_status` (mod.rs:2295), and the variant is the general-purpose write-rejection signal for any future write op too. A user invoking `clear status` from a follower will get an MCP `invalid_request` message that names "rebuild index" — that's confusing and steers debugging in the wrong direction. The unit test `test_write_db_follower_error_message_mentions_workspace_and_db` (workspace.rs:964) checks the workspace path and PID but not the op name, and `test_clear_status_returns_typed_error_on_follower` (mod.rs:3222) only asserts `msg.contains("read-only")`, so the regression isn't caught. Fix: reword the message generically (e.g. `"the code-context database is held read-only by this process; the writable connection is owned by the leader (pid …, db …) — usually an MCP server running for an agent session in {workspace_root}. Stop that session and rerun the op through it."`), or add an `op_name: &'static str` field on the variant and have each call site pass `"rebuild index"` / `"clear status"`. Either way, also extend the `clear_status` test to assert the message does NOT misname the op. **Fixed**: reworded the `#[error]` Display message generically — no longer names a specific op. Extended `test_clear_status_returns_typed_error_on_follower` to assert the message does NOT contain `"rebuild index"`. All four follower tests pass.

### Nits
- [x] `swissarmyhammer-leader-election/src/election.rs:160-162` — Docstring on `write_leader_pid` says it is "responsible for truncating to the new PID's exact length before writing." That's slightly off — the body calls `set_len(0)` then `writeln!`, i.e. truncates to zero and rewrites, which is correct but not what the prose claims. Reword to "truncating any prior content and rewriting from offset 0 so a shorter PID does not leave trailing bytes from a longer one." **Fixed**: reworded the doc-comment as suggested.
- [x] `swissarmyhammer-code-context/src/error.rs:34-42` — The variant doc-comment refers to "the code-context database is opened read-write only by the leader. When any other process opens the same workspace it joins as a follower with a read-only connection." Consider adding a one-line cross-reference to `Workspace::write_db` so future readers find the call site that produces this variant from the type definition. **Fixed**: added a cross-reference paragraph pointing to `CodeContextWorkspace::write_db` as the single producing call site.