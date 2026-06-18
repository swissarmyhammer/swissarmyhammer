---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvdehjbzc0syk73x3q47e6te
  text: |-
    Refinement from owner (2026-06-18): leadership/root resolution must resolve to the **outermost** workspace — the *first `sah` walking up from the working directory* — not merely the nearest workspace root. Subagents and worktrees that run in subdirectories must elect/join the SAME top-level leader and call UP to it (via the `^ref6nj4` follower→leader request API), rather than each electing a local leader and spawning their own rust-analyzer. This is the explicit intent behind "analyzers start at the root / first sah; subagents call back up." Confirm the election key derives from this outermost root and coincides with the index-DB leadership key.

    Also: the orphaned-rust-analyzer cleanup is tracked separately in ^x6m3jpz (probe leak + kill-on-exit). This task owns only the leader-handoff orphan case (try_promote re-spawn); coordinate the "no orphans" acceptance with that task and don't duplicate the lifecycle fix here.
  timestamp: 2026-06-18T13:25:49.567520+00:00
- actor: claude-code
  id: 01kvdnj7f2pmtknwqh47ksd29g
  text: |-
    ROOT CAUSE PINNED (2026-06-18, verified against the *running* binary = main @389362, `~/.cargo/bin/sah` built Jun 17):

    The exact defect is in `swissarmyhammer-tools/src/mcp/server.rs::do_initialize_code_context` (main line 363; identical on the diagnostic branch — NOT yet fixed there). Order of operations:
    1. `let lsp_handle = Self::spawn_lsp_supervisor(workspace_root)` ← spawns rust-analyzer (LspSupervisorManager::start → LspDaemon) **unconditionally**
    2. `let ws = open_workspace(...)` ← leadership is only DETERMINED here, AFTER the supervisor was already spawned
    3. `start_workers_if_leader(...)` / `start_lsp_workers_if_leader(...)` ← `is_leader()` gates ONLY the indexing/symbol-collection workers, never the LSP server process

    So leader election already works (it gates indexing) but the LSP **server spawn sits before and outside the gate**. Every `sah serve` process therefore launches its own rust-analyzer. Because `sah` is a stdio MCP server, every Claude agent AND every headless `claude --print` subagent spawns its own `sah serve`, so a subagent fan-out → N `sah serve` → N rust-analyzers over the same tree + shared cargo `target/` → resource thrash. Observed live: one interactive session fanned out ~27 subagents, producing ~20 rust-analyzers in minutes; this is what regenerates the orphan pile (cross-ref `^x6m3jpz`). The subagents' apparent "hang" is downstream — they complete MCP init (proven: each had already spawned its RA) and then stall on their turn/API; the sah-side bug is the unconditional spawn, not a sah deadlock.

    FIX (this task), make the ordering correct:
    - Determine leadership FIRST, then spawn the supervisor/rust-analyzer ONLY if leader. Move the spawn from the unconditional position to inside the leader branch + the promotion path (the reelection loop already re-runs worker startup on promotion — the supervisor spawn must join it).
    - Followers spawn NOTHING and route live LSP requests up to the leader via `^ref6nj4`.
    - Combine with the outermost-root keying (prior comment) so nested subagents/worktrees share ONE leader.

    DEPLOY NOTE: the running `sah` is from main, so this only takes effect after the fix lands AND `just sah` + redeploy. Verifying against a stale binary will mislead.
  timestamp: 2026-06-18T15:28:31.202458+00:00
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