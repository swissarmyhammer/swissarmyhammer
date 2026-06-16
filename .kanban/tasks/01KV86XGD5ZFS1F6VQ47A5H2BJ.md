---
assignees:
- claude-code
position_column: todo
position_ordinal: a980
project: diagnostics
title: 'Leader-per-workdir: only the leader spawns the LSP session'
---
## What
The one-client invariant holds per process; across processes (subagents, a CLI run, the editor's SAH) it needs arbitration or N processes spawn N rust-analyzers over the same tree. Reuse `swissarmyhammer-leader-election` (flock-based, keyed by workspace root MD5 hash, `LeaderElection::elect` → `ElectionOutcome::Leader|Follower`, `LeaderGuard`/`FollowerGuard`, `try_promote`, `peek_leader_pid`, `socket_path`).

- Gate LSP-session ownership on leadership: only the leader spawns the stdio LSP child and owns the one `LspSession`, single `initialize`, and id space. LSP servers (rust-analyzer, sourcekit-lsp, clangd) speak stdio only — one client, no listener — so only the leader ever touches the server.
- Key the election on the canonical workspace root (align with the existing key used by code-context's leader usage so LSP and index leadership coincide).
- Followers must NOT construct an `LspDaemon`/session; they will reach the leader via the request API (next task). For this task, a follower simply declines to spawn and surfaces a typed "not leader" state.
- **Handoff:** on leader exit, `try_promote` elects a follower which re-spawns + re-indexes (a cold start — acceptable since handoff only happens when the owning process exits). Surviving handoff without re-index needs a detached daemon — explicitly DEFERRED.

## Depends on
- "Add single owned LSP session with shared open-document set in swissarmyhammer-lsp"

## Acceptance Criteria
- [ ] Only the elected leader spawns the LSP child + owns the session; followers spawn nothing.
- [ ] Election keyed on canonical workspace root, coinciding with the existing code-context leadership key.
- [ ] Leader exit → `try_promote` path re-establishes a leader (cold re-spawn); no orphaned rust-analyzer processes.

## Tests
- [ ] `cargo test -p swissarmyhammer-lsp` (or an integration test): two in-process `LeaderElection` instances on the same workspace root — assert exactly one becomes leader/spawns, the other is a follower that spawns nothing; drop the leader, assert the follower can `try_promote`.

## Workflow
- Use `/tdd`. Note overlap with the `rebuild-index` project's deferred follower-IPC; coordinate so the keying matches. #diagnostics