---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m53gkrf8tt7c2ryb7mnf4x1y
  text: |-
    ### note — from the Swift port check
    - `shutdown()` must wait for a `start()` that is in progress. If it does not, a running start can add daemons after shutdown returns.
    - When the supervisor stops at the restart limit, log the error one time. Do not log a warning again at each health interval.
  timestamp: 2026-10-16T23:26:32.424187+00:00
position_column: todo
position_ordinal: ffff80
title: Stop the LSP on leader step-down and limit crash restarts
---
## Problem
Many sourcekit-lsp processes can run for one workspace, and each new start does a full index again.
- `step_down()` (`crates/swissarmyhammer-code-context/src/workspace.rs`) sets only the worker shutdown flag. It does not stop the LSP supervisor. The health loop and the diagnostics watcher continue. The old LSP process continues next to the LSP of the new leader.
- When a process becomes leader again, `start_indexing_workers_after_promotion` (`crates/swissarmyhammer-tools/src/mcp/server/code_context.rs`) makes a new supervisor. `LSP_SUPERVISOR.set(...)` fails because it is a `OnceLock` (`support.rs`). The new supervisor is dropped, and `kill_on_drop` kills the new LSP after it starts to index.
- In `crates/swissarmyhammer-lsp/src/daemon.rs`, `consecutive_failures` goes back to 0 after each good `initialize`. A server that initializes and then crashes restarts with no limit, about every 60 s.
- Nothing calls `LspSupervisorManager::shutdown()` when sah exits.

## Fix
- On step-down, shut down the LSP supervisor and stop the diagnostics watcher.
- On promotion, use one supervisor slot that sah can replace (for example `Mutex<Option<...>>` or `ArcSwap`), not a `OnceLock` that fails silently.
- Reset `consecutive_failures` only after the server stays healthy for a minimum time (for example 5 minutes). Use a backoff between restarts. Stop at the limit and log an error.
- Call supervisor shutdown when `sah serve` exits.

## Acceptance criteria
- After a step-down, no LSP child process of the old leader stays alive.
- Promotion after step-down gives one working LSP, and that LSP is not killed.
- A server that crashes after each good `initialize` stops after the limit.

## Tests
- Unit tests in `daemon.rs` for the failure counter and backoff, with a fake server command that exits after `initialize`.
- A test for step-down, then promotion, that checks that the supervisor slot holds a live supervisor. #lsp-fix #bug