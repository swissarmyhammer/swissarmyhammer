---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc880
project: ai-panel
title: Download lock wedges the app on macOS when the owning process died (stale-lock recovery is Linux-only)
---
## DONE (2026-05-29)

Fixed in code (no manual lock-poking): a download lock left by a crashed/killed process is now reclaimed in ~10s cross-platform, instead of wedging the app for up to 5 minutes on macOS.

## Root cause (`crates/model-loader/src/download_lock.rs`)
`is_stale_lock` only detected a dead owning process on Linux (`#[cfg(target_os="linux")]` via `/proc/<pid>`). On macOS the only recovery was the 5-min age threshold, so a stale `status=downloading` lock made every new run hang. `LockGuard::drop` deliberately leaves the lock file behind (it carries completion status), so a crash always leaves one.

## Fix
- `process_is_alive(pid)`: cross-platform liveness via `libc::kill(pid, 0)` on all unix (0/EPERM = alive, ESRCH = dead); non-unix falls back to "alive" + age threshold. Added `libc` to model-loader unix deps.
- `parse_lock_pid` + pure `stale_decision(content, age, owner_alive)`:
  - completed → never stale
  - owner alive → never stale (don't steal an active download of a huge file; the acquire loop's `MAX_WAIT_DURATION` 10-min cap is the backstop)
  - owner dead → stale after a 10s grace (reclaimed fast)
  - owner unknown → 5-min age fallback
- `is_stale_lock` now resolves age + liveness and defers to `stale_decision`. This also fixes a latent bug where a *live* slow download would be wrongly stolen after 5 minutes.

## Verification
- 26 download_lock tests pass, incl. new: `stale_decision` matrix (completed/alive/dead/unknown × age), `process_is_alive` (self alive, reaped-child dead, pid<=0 dead), and end-to-end `is_stale_lock_reclaims_dead_owner_quickly` (dead-owner lock backdated past the 10s grube via `libc::utimes` → stale) and `is_stale_lock_keeps_live_owner_even_when_old` (self-owned lock backdated past 5 min → NOT stale). No network/model needed.
- fmt clean, clippy 0; `llama-agent` + `llama-embedding` check green.

## Acceptance criteria
- [x] Dead-owner `status=downloading` lock reclaimed on macOS without the 5-min wait.
- [x] Live owner's lock never stolen (no false steal).
- [x] `completed` locks still short-circuit to the cached path.
- [x] Deterministic unit tests for the decision matrix + liveness primitive; no real network/model.

## Follow-up (separate, noted)
After clearing OUR lock, the run surfaced an hf-hub *blob-level* lock (`blobs/<sha>.lock`) → `Lock acquisition failed … Retrying`. That is hf-hub's internal lock (a different layer); filed mentally as a separate investigation if it recurs after this fix + a clean app restart.