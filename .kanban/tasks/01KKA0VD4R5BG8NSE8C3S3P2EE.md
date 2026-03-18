---
position_column: done
position_ordinal: be80
title: Gate query operations on indexing readiness
---
## What

The spec says query operations (`grep code`, `search code`, `get symbol`, etc.) should block until the relevant indexing layer is complete, returning a progress notification. Currently they return empty results during indexing, which causes the agent to conclude symbols don't exist.

`check_blocking_status()` exists in `swissarmyhammer-code-context/src/blocking.rs` but is query-only — never integrated into the query path.

**Key files:**
- `swissarmyhammer-code-context/src/blocking.rs` — `check_blocking_status()` (exists, unused)
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — query execution functions

**Approach:**
1. Before executing query ops, call `check_blocking_status()`
2. If `NotReady`, return a progress message instead of empty results
3. `get status` remains non-blocking (always returns immediately)
4. Tree-sitter ops block only on TS readiness; LSP ops block on LSP readiness

## Acceptance Criteria
- [ ] `grep code` returns progress message during indexing, not empty results
- [ ] `get symbol` returns progress message during indexing
- [ ] `get status` always returns immediately (exception)
- [ ] Once indexed, queries return normally

## Tests
- [ ] Unit test: NotReady status → progress message returned
- [ ] Unit test: Ready status → query executes normally
- [ ] `cargo test -p swissarmyhammer-tools` passes