---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvdfan4eb5b2j3fyasm8rdat
  text: |-
    Additional scope from na6cvh0 double-check: this e2e must also validate the diagnostics MCP tool's path-space handling end to end. The tool feeds `diagnose` ABSOLUTE paths (LSP side) while querying the code-context blast-radius index in REPO-RELATIVE space (see diagnostics/mod.rs absolutize/relativize). Two things to verify here that unit tests cannot:
    1. A working-tree edit under a repo whose root != process CWD is actually reported (catches relative-vs-absolute path bugs in `check working`/`check sha`).
    2. Symlink canonicalization: rust-analyzer may emit canonicalized URIs (e.g. macOS /var -> /private/var). If the tool's `repo.join(rel)` absolute paths don't match the server's canonicalized URIs, diagnostics won't be found — confirm/repair (likely canonicalize the repo root in the tool's repo_root()).
  timestamp: 2026-06-18T13:39:31.598259+00:00
position_column: todo
position_ordinal: af80
project: diagnostics
title: 'Integration test (rust-analyzer): diagnose([A]) reports A + broken dependent B, not clean C'
---
## What
End-to-end integration test for `swissarmyhammer_diagnostics::diagnose` against a real `rust-analyzer`, gated at runtime on `which::which("rust-analyzer")` (skip+green when absent, matching `crates/swissarmyhammer-lsp/tests/session_rust_analyzer.rs`).

Split out of task ^9fq036d (diagnose core API): the core `diagnose`, the `BlastRadiusDependents` resolver, and the broken-vs-clean selection logic are implemented and covered by model-free unit tests (`crates/swissarmyhammer-diagnostics/src/diagnose.rs`). This task is the heavier e2e validation that needs both rust-analyzer AND a populated code-context index, so it was deferred to keep the core task focused.

## Why it's non-trivial
The blast radius (B depends on A) is read from the code-context SQLite index (`lsp_call_edges`/`lsp_symbols`). The test must therefore stand up the code-context indexing pipeline (symbol + call-edge collection via `lsp_communication::collect_and_persist_*`) against the fixture workspace so `get_blastradius("src/a.rs", max_hops=1)` returns B — this is the bulk of the setup, beyond just starting rust-analyzer.

## Plan
- Temp cargo workspace: `src/a.rs` (defines a fn), `src/b.rs` (calls A's fn — a real call edge), `src/c.rs` (independent, clean).
- Start `LspDaemon` → `session`; index the workspace into a code-context DB so call edges A←B exist.
- Edit A to break it (change/remove the fn B calls) and sync; `diagnose(session, ["src/a.rs"], config, BlastRadiusDependents::new(&conn), TokioTimer)`.
- Assert: report contains A's error AND B as a broken dependent; does NOT contain clean file C.

## Acceptance Criteria
- [ ] Runtime-gated; green-skips without rust-analyzer.
- [ ] Asserts A error + B-as-broken folded in, C excluded.

#diagnostics