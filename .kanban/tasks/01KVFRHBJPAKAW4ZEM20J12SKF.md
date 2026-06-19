---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvfrjh6679p9nqnpwbqyqhcg
  text: 'Picked up directly (owner confirmed: canonicalize is the right fix; worktrees correctly stay separate). Driving implement → review → commit now.'
  timestamp: 2026-06-19T10:59:35.750425+00:00
- actor: claude-code
  id: 01kvfrq3pq93h2eqxgn583tbg7
  text: |-
    Chokepoint chosen: LeaderElection::with_config (leader-election crate). Reason: blast-radius shows TWO production election entries derive the rust-analyzer/index key from a workspace root — swissarmyhammer-code-context/src/workspace.rs::open AND swissarmyhammer-treesitter/src/unified.rs::open_internal (plus lsp supervisor test helper). Canonicalizing only at the code-context boundary would leave the treesitter path split — the bug just relocates. with_config is the one place every consumer flows through. It canonicalizes ONLY the value fed into hash_path + the stored workspace_root field; callers compute their own DB/context dirs from their own raw root, so DB paths are untouched. Fix: `let raw=...to_path_buf(); let workspace_root = fs::canonicalize(&raw).unwrap_or(raw);` — graceful fallback covers non-existent paths (existing tests use /some/workspace, /ws which canonicalize-fail and fall back to raw, staying green).

    Leader/follower agreement: diagnostics repo_root() already canonicalizes (^affvtqf); canonicalize is idempotent so canonicalize(canonical)==canonical → follower (diagnostics) and leader (code-context/treesitter) derive the SAME lock/socket. Worktrees: distinct canonical dirs stay distinct → separate leaders (regression test proves it).

    TDD: symlink test went RED first (left /var/...416844... vs right ...dc194959... differing lock_paths), then GREEN after canonicalize. swissarmyhammer-leader-election: 91 passed, 0 failed. Running downstream crates + clippy/fmt next.
  timestamp: 2026-06-19T11:02:05.783962+00:00
- actor: claude-code
  id: 01kvfrw3qcwhzt19tbn088nwnt
  text: |-
    Verification (fresh): swissarmyhammer-leader-election 91 passed/0 failed; clippy --all-targets -D warnings clean; cargo fmt --check clean. Downstream election consumers all green: code-context 628+26+2, lsp leader_gated_lsp_session 1, treesitter --lib 353.

    Safety check on the stored workspace_root field change: LeaderElection::workspace_root() is read ONLY inside election.rs tests (no external caller — verified by grep). The CodeContextWorkspace/UnifiedContext workspace_root() accessors return THEIR OWN stored raw field, not the election's, so `assert_eq!(ws.workspace_root(), dir.path())` tests are unaffected. test_election_new (raw /some/workspace via canonicalize-fail fallback) still passes. Awaiting double-check verdict.
  timestamp: 2026-06-19T11:04:49.644398+00:00
- actor: claude-code
  id: 01kvfrzbhcewnqj0k0gza0583f
  text: 'really-done PASS. Fresh verification: leader-election 91 passed/0 failed; clippy --all-targets -D warnings clean; cargo fmt --check clean. Downstream election consumers green (code-context 628+26+2, lsp leader_gated 1, treesitter --lib 353). Adversarial double-check verdict: PASS — confirmed single chokepoint (hash_path only called from with_config; all consumers route through new/with_config), graceful fallback exercised by test_election_new, symlink test is genuine RED→GREEN (not a tautology), no over-collapse, and no production caller reads LeaderElection::workspace_root() relying on the raw form. Moved to review.'
  timestamp: 2026-06-19T11:06:35.948640+00:00
- actor: claude-code
  id: 01kvfs6ndsvywfmxnednsm5nem
  text: |-
    GREEN + verified. Implementation: one-line canonicalize in LeaderElection::with_config (`let raw = ...to_path_buf(); let workspace_root = fs::canonicalize(&raw).unwrap_or(raw);`) feeding both hash_path and the stored field. Only file touched: crates/swissarmyhammer-leader-election/src/election.rs.

    Blast-radius check on workspace_root() readers: confirmed NO production caller reads LeaderElection::workspace_root() (grep, excl. election.rs, returns only CodeContextWorkspace/UnifiedContext wrappers, which store their OWN raw root — unaffected). LeaderElection::workspace_root() is read only by election.rs tests; test_election_new uses /some/workspace which canonicalize-fails → falls back to raw → still passes.

    Tests: leader-election 91 passed / 0 failed (incl. new test_symlink_equivalent_roots_derive_same_lock_path [RED→GREEN] + test_distinct_dirs_derive_distinct_lock_paths). Downstream election consumers all green: code-context 628+26+2, treesitter --lib 353, lsp leader_gated_lsp_session 1. clippy -p swissarmyhammer-leader-election --all-targets -D warnings: 0 warnings. cargo fmt --check: clean (exit 0). really-done double-check critic: VERDICT: PASS. Moving to review.
  timestamp: 2026-06-19T11:10:35.449369+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffca80
project: diagnostics
title: Canonicalize the workspace root before deriving the LSP/index election key
---
## Why (evidence, 2026-06-19 investigation)
The leader-election key that gates "one rust-analyzer per workspace root" is an MD5 of a **raw, un-canonicalized** path string. So two `sah serve` processes that resolve the SAME physical directory via different string forms elect SEPARATE leaders and spawn SEPARATE rust-analyzers:
- macOS `/var/...` vs `/private/var/...` (and `/tmp` → `/private/tmp`) symlink forms → different MD5 → two leaders.
- Any other symlinked/relative form of the same dir.

Evidence:
- `crates/swissarmyhammer-leader-election/src/election.rs` `hash_path` MD5s `path.to_string_lossy()` with no normalization; `with_config` derives `lock_path`/`socket_path` from that hash.
- `crates/swissarmyhammer-code-context/src/workspace.rs::open` passes the raw `workspace_root` straight to `LeaderElection::with_config` (no canonicalize).
- `crates/swissarmyhammer-tools/src/mcp/server.rs::do_initialize_code_context` / `resolve_workspace_root` (`:1582`) and `open_workspace` (`code_context/mod.rs:1216`) use `work_dir`/`current_dir()` raw.
- Tellingly, the diagnostics tool's `repo_root()` was ALREADY fixed to canonicalize (^affvtqf), and the diagnose e2e test canonicalizes by hand (`mod.rs:997`) — proving the rest of the production path does not, so the two sides can disagree.

NOTE: git worktrees correctly remain SEPARATE (a worktree is a distinct checkout that needs its own rust-analyzer) — `find_git_repository_root_from` stopping at the worktree's `.git` file is desired behavior, NOT a bug. Canonicalization preserves that (distinct worktrees are distinct canonical dirs) while collapsing symlink-equivalent forms of the SAME dir to one leader.

## Fix
Canonicalize the resolved workspace root (`std::fs::canonicalize`, with a graceful fallback to the raw path if canonicalize fails — the dir normally exists) ONCE at the workspace-root resolution boundary, so EVERY consumer that derives the election key agrees. The leader-startup path (`do_initialize_code_context`/`resolve_workspace_root`), the `CodeContextWorkspace::open` path, and the diagnostics tool's `repo_root()` (already canonical) must all produce the SAME canonical root → same lock/socket → one leader. Pick a single chokepoint; do NOT canonicalize in some paths but not others (that just moves the split). Evaluate canonicalizing at root-resolution (narrow, code-context identity) vs inside `LeaderElection`/`hash_path` (broad, all election users) and justify the choice; reuse, don't add a parallel normalization.

## Acceptance Criteria
- [ ] The workspace root feeding the election lock/socket key is canonicalized at one consistent chokepoint; leader and follower opened against symlink-equivalent forms of the same dir derive the SAME lock_path/socket_path and elect ONE leader.
- [ ] Distinct git worktrees still elect distinct leaders (each gets its own rust-analyzer) — canonicalization must not collapse genuinely-different checkouts.
- [ ] No second/parallel path-normalization mechanism; existing callers (incl. the diagnostics `repo_root()` canonicalize) stay consistent.

## Tests
- [ ] Unit (model-free, <1s): two `LeaderElection`/`CodeContextWorkspace` instances opened with a real symlinked dir and its canonical target resolve to the SAME lock_path (and only one wins the flock). Use a tempdir + a symlink to it.
- [ ] Regression: two distinct dirs (or worktrees) still get distinct keys.

## Workflow
- Use `/tdd`. #diagnostics

## Review Findings (2026-06-19 06:15)

Reviewed `review working` (uncommitted delta = `crates/swissarmyhammer-leader-election/src/election.rs`, +97/-1: the canonicalize line plus two new tests + doc). Verdict: **clean — no actionable blockers in the change under review.**

### Engine findings — verified and refuted (out of scope / pre-existing)
- [x] Engine flagged a "blocker" path-traversal via `config.prefix` in the `{}-ts-{}.lock`/`.sock` format strings. REFUTED as a blocker for this task: that interpolation is pre-existing code unchanged by this diff, and `prefix` is a developer-supplied code constant (`"sah"`, `"code-context"`, `"lsp-leader-test"`), never request/attacker input — no traversal surface. Not introduced here, not in scope.
- [x] Engine flagged a "warning" on `config: config.clone()` (consume-then-clone). REFUTED as actionable: pre-existing line, unchanged by this diff; `config.base_dir()` is read before the struct literal and the struct also stores `config`, so the clone is a trivial pre-existing micro-allocation, not a defect introduced by this change.

### Confirmed (the load-bearing claims hold)
- [x] **Chokepoint correctness — leader + follower agree.** `with_config` canonicalizes once (`let raw = ...; let workspace_root = fs::canonicalize(&raw).unwrap_or(raw);`) and derives `lock_path`/`socket_path` from the canonical hash. Both production consumers — `CodeContextWorkspace::open` (workspace.rs: derives lock/socket from `election.lock_path()`/`socket_path()`) and `UnifiedContext::open_internal` (unified.rs: same) — take their paths exclusively from the election's canonicalized output, never an independently-computed raw path. No separate raw-path derivation anywhere on the follower side.
- [x] **Idempotent agreement with diagnostics `repo_root()` (^affvtqf).** `canonicalize` is idempotent → `canonicalize(canonical) == canonical`, so the already-canonical diagnostics follower and the canonicalizing leader derive the SAME key. No double-canonicalize divergence.
- [x] **Blast radius safe.** All election consumers route through `new`/`with_config` (code-context, treesitter `open_internal`, lsp supervisor test helper, diagnostics tests). `.unwrap_or(raw)` preserves exact prior behavior for non-existent paths (no panic) — exercised by `test_election_new`/`test_election_with_custom_config` (`/some/workspace` canonicalize-fails → falls back to raw, still green). No production caller reads `LeaderElection::workspace_root()` relying on the raw form; the only `.workspace_root()` readers in tools (`code_context/mod.rs`) read `CodeContextWorkspace`'s own stored root for a doctor report, not a lock-path comparison.
- [x] **Worktrees still split (no over-collapse).** `test_distinct_dirs_derive_distinct_lock_paths` opens two distinct tempdirs, asserts distinct `lock_path`s, and asserts both win their own election (two leaders). Distinct canonical dirs stay distinct → separate rust-analyzers. Meaningful.
- [x] **Symlink test genuine (RED→GREEN, not a tautology).** `test_symlink_equivalent_roots_derive_same_lock_path` creates a real `std::os::unix::fs::symlink` to a real tempdir, opens elections via the canonical path AND the symlink, asserts identical `lock_path`/`socket_path`, then asserts one Leader + one Follower against the same physical dir. It compares the two elections' outputs directly (does not canonicalize the expected value itself), so it would fail pre-fix on differing raw-string hashes.
- [x] **`to_string_lossy` hashing on canonical `PathBuf`.** `hash_path` operates on the canonical `PathBuf` unchanged; no TOCTOU or panic hazard introduced.