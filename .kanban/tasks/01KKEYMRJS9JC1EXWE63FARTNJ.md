---
depends_on:
- 01KKEYM9KDP3WDT0VVYRJKHMDT
position_column: done
position_ordinal: dc80
title: Add unit tests for lsp_degradation_notice() and verify notice presence/absence on operations
---
## What

When code_context tool operations return results and the LSP layer is unavailable (daemon in `NotFound` state), append a notice to the response telling the agent (and by extension the user) that results are tree-sitter only and how to install the missing LSP server.

**Files to modify:**
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — the dispatch function and individual execute functions

**Approach:**
1. Create a helper function `lsp_degradation_notice(workspace_root: &Path) -> Option<String>` that:
   - Checks the global `LSP_SUPERVISOR` for any daemons in `NotFound` state
   - Falls back to `doctor::run_doctor()` if supervisor isn't initialized yet
   - Returns `None` if all LSP servers are available
   - Returns a formatted notice string with install hints if any are missing
2. In the `call_tool` dispatch, after getting a successful result from any query operation (`get symbol`, `search symbol`, `list symbols`, `grep code`, `get callgraph`, `get blastradius`), check for degradation and append the notice to the response text
3. Do NOT add notices to `get status` (it already reports LSP state), `build status`, or `clear status`

**Example appended notice:**
```
---
Note: Code intelligence is limited to tree-sitter only. LSP server 'rust-analyzer' is not installed.
Install: rustup component add rust-analyzer
```

**Key integration point:** The `LSP_SUPERVISOR` static is already in this module (line 37). When daemons are in `NotFound` state, their `install_hint` from the YAML spec is available.

## Acceptance Criteria
- [ ] When an LSP server is missing, code_context query operations include a notice in the response with the install command
- [ ] When all LSP servers are running, no notice is appended (no noise)
- [ ] The notice appears on `get symbol`, `search symbol`, `list symbols`, `grep code`, `get callgraph`, `get blastradius`
- [ ] The notice does NOT appear on `get status`, `build status`, `clear status`

## Tests
- [ ] Unit test: verify `lsp_degradation_notice()` returns `None` when no supervisor is set
- [ ] Unit test: verify notice formatting includes server name and install hint
- [ ] `cargo test -p swissarmyhammer-tools` passes