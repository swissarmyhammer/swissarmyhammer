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
- actor: claude-code
  id: 01kvdqfmwa1wexzjybj604ff47
  text: 'Picked up by /finish $diagnostics batch. Driving this BEFORE ^ref6nj4 even though both show #READY: ref6nj4''s description declares "Depends on 7a5h2bj" but the kanban depends_on array is empty (known depends_on-clobber bug), so the ready flag understates the real ordering. The multiplexer (ref6nj4) dispatches onto "the one LspSession" that only exists once leadership gates the spawn — this task — so it must land first. Root cause is already pinned in the prior comment; implementing the leader-gated spawn reordering now.'
  timestamp: 2026-06-18T16:02:03.786474+00:00
- actor: claude-code
  id: 01kvdqkc5argzep389bxhxrbfg
  text: |-
    Research done. Confirmed:
    - Defect in server.rs::do_initialize_code_context — spawn_lsp_supervisor() runs UNCONDITIONALLY before open_workspace() decides leadership. is_leader() gates only TS/LSP indexing workers, never the rust-analyzer spawn.
    - resolve_workspace_root() already uses find_git_repository_root_from(), whose doc says it "finds the outermost enclosing repo" and handles worktrees. So the canonical/outermost-root keying the owner asked for already holds; code-context's LeaderElection (prefix "code-context") keys on this same root. The LSP spawn must coincide with THAT existing code-context leadership decision (ws.is_leader()), not a second competing election on a different lock — otherwise a process could win one lock and lose the other.

    Plan:
    1. New swissarmyhammer-lsp module `ownership.rs`: a typed `LspOwnership` (Leader{supervisor}/Follower) that gates the supervisor start on a leadership bool. Followers spawn NOTHING and surface a typed not-leader state; promotion re-runs the gated start. Adds dep on swissarmyhammer-leader-election so the acceptance test can drive it with two same-root LeaderElection instances.
    2. server.rs: gate the supervisor spawn on ws.is_leader() (initial path) AND wire it into start_indexing_workers_after_promotion (promotion path) so promotion re-spawns the supervisor. Followers spawn nothing.
    TDD: writing the failing swissarmyhammer-lsp test first (two same-root LeaderElection instances → exactly one leader spawns, follower spawns nothing; drop leader → follower try_promote succeeds).
  timestamp: 2026-06-18T16:04:05.930957+00:00
- actor: claude-code
  id: 01kvdr5eprdzfxj0qvnvvqqjst
  text: |-
    Implementation landed.

    Production fix (server.rs::do_initialize_code_context): leadership is now decided FIRST via open_workspace(); the LSP supervisor (spawn_lsp_supervisor → rust-analyzer et al.) is spawned ONLY in the leader branch. Followers spawn NOTHING — they log a typed not-leader state and only run the re-election poll loop. The promotion path (start_indexing_workers_after_promotion) now performs a cold re-spawn of the supervisor itself (a follower never spawned one), then starts LSP indexing workers off it and runs the health loop. Removed the now-dead wait_for_lsp_supervisor() helper and its two unused constants (LSP_SUPERVISOR_POLL_INTERVAL / _WAIT_TIMEOUT), since promotion no longer waits for a pre-existing supervisor.

    Keying: gated on the EXISTING code-context election (ws.is_leader()) keyed on resolve_workspace_root() = find_git_repository_root_from() (outermost enclosing repo, worktree-aware). So LSP-session ownership and index-DB ownership ride the SAME flock on the same root — they cannot split. I deliberately did NOT introduce a second/parallel LSP election (would be duplicate-but-different and could win one lock while losing the other).

    Acceptance test (per the card): crates/swissarmyhammer-lsp/tests/leader_gated_lsp_session.rs — two LeaderElection instances on the same workspace root; exactly one becomes Leader and spawns/owns an LspSupervisorManager, the other is a Follower that owns nothing; while the leader is alive the follower's try_promote returns None; drop the leader's guard → follower try_promote succeeds → cold-spawns its own supervisor. (swissarmyhammer-leader-election added as a dev-dependency of swissarmyhammer-lsp.)

    TDD note: red-green was first done against a transient LspOwnership type (watched it fail with the leader-gating stubbed out, then pass); I then removed that type in favor of reusing the existing LeaderElection + LspSupervisorManager directly (no duplicate mechanism), and the shipped acceptance test exercises that real composition.

    Verification:
    - cargo test -p swissarmyhammer-lsp → 218 lib + 1 leader_gated_lsp_session + 1 session_rust_analyzer + 1 doctest, all pass.
    - cargo test -p swissarmyhammer-tools --lib → 1075 passed, 0 failed.
    - cargo clippy -p swissarmyhammer-lsp -p swissarmyhammer-tools --all-targets -- -D warnings → clean.
    - cargo fmt --check → clean.
    - Cargo.lock diff = 1 line (the new dev-dep edge).

    NOT done (out of scope, tracked elsewhere): follower→leader request multiplexer (^ref6nj4); orphaned-rust-analyzer kill-on-exit lifecycle (^x6m3jpz). This task does not introduce new orphans — followers spawn no rust-analyzer at all, and promotion is a cold re-spawn only after the prior leader exits.
  timestamp: 2026-06-18T16:13:58.360824+00:00
- actor: claude-code
  id: 01kvdrk3smj5cahq11sxjqs12g
  text: |-
    Adversarial double-check returned PASS on production code, REVISE on the test only (two findings, both test-quality). Addressed both:

    Finding #1 (lsp test reimplemented the gate inline and never drove the real server path → a regression moving the spawn back outside `if is_leader` would stay green; doc overstated coverage):
    - Extracted the production gate into a single named seam: McpServer::spawn_lsp_supervisor_if_leader(is_leader, root) -> Option<JoinHandle<...>> in server.rs. do_initialize_code_context now branches on its Some/None result. This is the ONLY startup site that spawns the supervisor.
    - Added a TDD'd unit test in swissarmyhammer-tools (mcp::server::tests::test_lsp_supervisor_spawn_is_leader_gated): asserts follower (is_leader=false) → None (spawns nothing), leader (true) → Some. Watched it RED (stubbed the gate to always-spawn → "a follower must not spawn the LSP supervisor" panic), restored → GREEN. This directly guards the exact regression.
    - Rewrote the lsp test doc-comment to claim only what it proves (the election semantics the gate keys on) and pointed at the tools-side gate test.

    Finding #2 (four tautological Option-bookkeeping assertions in the lsp test):
    - Removed them. The lsp test (renamed one_leader_per_root_follower_can_promote_only_after_leader_exits) now asserts only the genuinely meaningful, non-tautological facts: first elector wins / second follows; follower try_promote → None while leader holds the flock; → Some after the leader drops; and the leader-side supervisor build that follows a won election (daemon_names empty on an empty workspace).

    Verification (all fresh, green):
    - cargo test -p swissarmyhammer-lsp → 218 lib + leader_gated_lsp_session + session_rust_analyzer + doctest, 0 failed.
    - cargo test -p swissarmyhammer-tools --lib mcp::server:: → 38 passed, 0 failed (incl. the new gate test).
    - cargo clippy -p swissarmyhammer-lsp -p swissarmyhammer-tools --all-targets -- -D warnings → clean.
    - cargo fmt --check → clean.
  timestamp: 2026-06-18T16:21:25.940944+00:00
- actor: claude-code
  id: 01kvds5bs3kz97xr2bxv45gatn
  text: |-
    Adversarial double-check (really-done advisory gate) ran and its finding was adopted: the integration test alone proved only the ELECTION semantics, not the leader-only-spawn gate itself. Resolution — extracted the gate into a testable seam McpServer::spawn_lsp_supervisor_if_leader(is_leader, root) -> Option<JoinHandle> (the single point the startup path spawns the supervisor; follower → None), routed do_initialize_code_context through it, and added a focused unit test server::tests::test_lsp_supervisor_spawn_is_leader_gated asserting follower→None / leader→Some. The integration test (crates/swissarmyhammer-lsp/tests/leader_gated_lsp_session.rs) was refined to lock the election semantics (one leader per root; follower can't win while leader holds the flock; follower wins + cold-spawns only after leader exits) and now cross-references the tools unit test.

    Also DRY'd the await→drain→health-loop shell shared by initial-leader startup and the post-promotion cold re-spawn into McpServer::spawn_drain_supervisor_and_health_loop (closure-parameterized worker start), removing the copy-paste.

    Final verification (all integrated, fully green):
    - cargo test -p swissarmyhammer-lsp → 218 lib + 1 acceptance (leader_gated_lsp_session) + 1 session_rust_analyzer + 1 doctest, 0 failed.
    - cargo test -p swissarmyhammer-tools --lib → 1076 passed (incl. test_lsp_supervisor_spawn_is_leader_gated), 0 failed.
    - cargo clippy -p swissarmyhammer-lsp -p swissarmyhammer-tools --all-targets -- -D warnings → clean.
    - cargo fmt --check → clean.
  timestamp: 2026-06-18T16:31:23.939991+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffbf80
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
- [x] Only the elected leader spawns the LSP child + owns the session; followers spawn nothing.
- [x] Election keyed on canonical workspace root, coinciding with the existing code-context leadership key.
- [x] Leader exit → `try_promote` path re-establishes a leader (cold re-spawn); no orphaned rust-analyzer processes.

## Tests
- [x] `cargo test -p swissarmyhammer-lsp` (or an integration test): two in-process `LeaderElection` instances on the same workspace root — assert exactly one becomes leader/spawns, the other is a follower that spawns nothing; drop the leader, assert the follower can `try_promote`.

## Workflow
- Use `/tdd`. Note overlap with the `rebuild-index` project's deferred follower-IPC; coordinate so the keying matches. #diagnostics

## Resolution
Fix in `server.rs::do_initialize_code_context`: leadership is decided first (via `open_workspace` → code-context's flock election keyed on `find_git_repository_root_from` = outermost repo); the supervisor spawn is gated through the single seam `McpServer::spawn_lsp_supervisor_if_leader(is_leader, root)` (leader → `Some(handle)`, follower → `None`, spawns nothing). Promotion path cold-re-spawns the supervisor. LSP and index leadership coincide because both ride the SAME flock on the same root (no second election). Tests: `swissarmyhammer-lsp/tests/leader_gated_lsp_session.rs` (election semantics + leader-side supervisor build) and `swissarmyhammer-tools` `mcp::server::tests::test_lsp_supervisor_spawn_is_leader_gated` (TDD red-green on the gate). Out of scope (separate tasks): follower→leader request multiplexer (^ref6nj4), orphan kill-on-exit lifecycle (^x6m3jpz).