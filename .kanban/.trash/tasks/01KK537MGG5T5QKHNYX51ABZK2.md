---
assignees:
- assistant
position_column: done
position_ordinal: o9
title: 'swissarmyhammer-lsp: daemon lifecycle (spawn, health, restart, shutdown)'
---
## What
Implement the `LspSupervisor` — spawns LSP child processes, runs the initialize handshake, health-checks on interval, restarts with exponential backoff, and shuts down gracefully.

Files: `swissarmyhammer-lsp/src/supervisor.rs`, `swissarmyhammer-lsp/src/daemon.rs`

Spec: `ideas/code-context-architecture.md` — "Daemon lifecycle" section.

## Acceptance Criteria
- [ ] `LspSupervisor::start(specs, workspace_root)` spawns relevant servers based on `detect_projects()`
- [ ] Binary existence check via `$PATH` lookup before spawn; logs `install_hint` at `warn!` if missing
- [ ] `initialize` handshake with `initializationOptions` from spec, respects `startup_timeout`
- [ ] Health check loop: sends lightweight request every `health_check_interval`, checks `child.try_wait()` for unexpected exit
- [ ] Restart policy: exponential backoff 1s→60s cap, 5 consecutive failures stops retrying
- [ ] `build status` with `layer: lsp` force-restarts failed servers (resets backoff)
- [ ] Graceful shutdown: `shutdown` + `exit`, then `SIGKILL` after 5s
- [ ] Per-server state queryable: `Running(pid, uptime)`, `Failed(attempts, last_error)`, `NotFound(install_hint)`

## Tests
- [ ] Unit test: backoff calculation (1, 2, 4, 8, 16, 32, 60, 60)
- [ ] Unit test: state transitions (Starting → Running, Starting → Failed, Failed → Starting)
- [ ] Integration test: spawn a mock LSP server (simple stdin/stdout echo), verify initialize handshake
- [ ] Integration test: kill mock server, verify restart with backoff
- [ ] `cargo test -p swissarmyhammer-lsp`