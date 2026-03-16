---
position_column: done
position_ordinal: s6
title: 'code_context tool: status operations + blocking + hints'
---
## What
Implement `get status` (health report with per-server LSP state), `build status` (trigger reindex), `clear status` (wipe and rebuild). Add blocking-during-indexing logic and next-step hints on all operation responses.

Files: `swissarmyhammer-code-context/src/ops/status.rs`, `swissarmyhammer-code-context/src/blocking.rs`, `swissarmyhammer-code-context/src/hints.rs`

Spec: `ideas/code-context-architecture.md` — "get status", "build status", "clear status", "Blocking during initial index", "Next-step hints" sections.

## Acceptance Criteria
- [ ] `get status` returns: mode, file counts, TS/LSP indexed %, dirty files, chunk/edge counts, per-LSP-server state (Running/Failed/NotFound with details)
- [ ] `get status` always returns immediately (never blocks), even mid-index
- [ ] `build status` triggers full reindex of specified layer(s), force-restarts failed LSP servers
- [ ] `clear status` wipes index DB and starts fresh
- [ ] All query operations block until relevant layer indexed, with progress notification
- [ ] Every operation response includes `hint` field with next-step suggestion

## Tests
- [ ] Unit test: `get status` on fresh DB shows 0% indexed
- [ ] Unit test: `get status` shows LSP server states (mock Running + Failed)
- [ ] Unit test: blocking waits until `ts_indexed` count matches total files
- [ ] Unit test: hints are non-empty strings for each operation
- [ ] `cargo test -p swissarmyhammer-code-context`