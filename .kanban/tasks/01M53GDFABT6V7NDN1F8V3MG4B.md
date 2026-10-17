---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m54mdsg2rv133m2gw3x7awqn
  text: |-
    Research done.
    - Dependency direction: code-context -> lsp, diagnostics -> code-context + lsp, tools -> all. The lsp crate cannot use IgnoreFilter.
    - `IgnoreFilter::is_ignored` already returns true for a path outside the root and for `..` components. So one rule set covers "ignored" and "outside root".
    - Plan: follow the existing `StderrFilter` injection seam. Add `PathFilter = Arc<dyn Fn(&Path) -> bool + Send + Sync>` in the lsp crate. `LspSession` holds it in the shared inner (all clones see it). `LspSession::open` and `sync_open` check it first and return `Ok(DocumentSync::Skipped)` with no wire traffic. `LspDaemon::set_path_filter` and `LspSupervisorManager::with_path_filter` pass it on. The tools crate installs a filter built from `IgnoreFilter` (code-context) at supervisor start.
    - Callers of `sync_open` that send a request after it must stop on Skipped: lsp_worker (mark row lsp_indexed = 1), layered_context `sync_document` (fall back to index layers), diagnostics request_api (2 sites), watcher `refresh_file`, diagnose (skip the pull).
  timestamp: 2026-10-17T09:52:25.602452+00:00
- actor: claude-code
  id: 01m54n216q5fw3jj0x4rn0dafm
  text: |-
    Implementation landed (TDD, each new test seen RED first).
    - swissarmyhammer-lsp: new `PathFilter` and `DocumentSync { Synced, Skipped }`. `LspSession::set_path_filter` (shared by all clones), public `skips_path`. `open` and `sync_open` return `Result<DocumentSync, LspError>` and send nothing for a skipped path, also with no live client. `LspDaemon::set_path_filter`, `LspSupervisorManager::with_path_filter` + private `new_daemon` that applies both filters.
    - code-context: `IgnoreFilter::into_path_filter` (one rule set: ignored paths and paths outside the root). Worker: `FileIndexOutcome { Indexed, ClientUnavailable, Skipped }`; a skipped row is marked lsp_indexed = 1 and is not counted. `LayeredContext::sync_document` maps Skipped to Ok(None) (index fallback).
    - diagnostics: watcher `refresh_file` sends no pull on Skipped; `diagnose` skips filtered paths before read/sync/pull; leader `dispatch` answers `SKIPPED_DOCUMENT_RESPONSE` (JSON null) for a filtered document and sends no request; client `lsp_request_with_document` / `lsp_multi_request_with_document` now give `Option` (None = skipped).
    - tools: supervisor gets `IgnoreFilter::load(root).into_path_filter()`; follower `route_one` / `route_multi` pass None through.
    - ARCHITECTURE.md: one sentence added next to the stderr-filter seam.
    - Note: the tools wiring line has no own unit test; the supervisor -> daemon -> session path and the filter itself are tested in their crates.
    - Note: the follower IPC skip uses JSON null as the sentinel; a real response is always an object or an array.
  timestamp: 2026-10-17T10:03:28.855610+00:00
- actor: claude-code
  id: 01m54n2a4spg0y73ytak19j3zt
  text: |-
    ### implement — changed
    - evidence: 15 files — crates/swissarmyhammer-lsp/src/{session.rs,daemon.rs,supervisor.rs,lib.rs}; crates/swissarmyhammer-code-context/src/{ignore_filter.rs,lsp_worker.rs,layered_context.rs}; crates/swissarmyhammer-diagnostics/src/{request_api.rs,watcher.rs,diagnose.rs,lib.rs}, crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs; crates/swissarmyhammer-tools/src/mcp/server/code_context.rs, crates/swissarmyhammer-tools/src/mcp/tools/code_context/leader_route.rs; ARCHITECTURE.md. `cargo nextest run -p swissarmyhammer-lsp -p swissarmyhammer-code-context -p swissarmyhammer-diagnostics -p swissarmyhammer-tools`: 2608 passed, 0 failed. `cargo clippy ... --all-targets -- -D warnings`: clean. `cargo fmt --all`: done.
    - next: /review
  timestamp: 2026-10-17T10:03:38.009287+00:00
- actor: claude-code
  id: 01m54nczznhc8yqdpjrtxbs0rs
  text: |-
    ### test — red
    - evidence: `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-lsp) | rdeps(swissarmyhammer-code-context)'` — 5226 tests run, 5225 passed, 1 failed, 0 skipped (1 leaky: swissarmyhammer-validators review::scope::fixtures::tests::a_scope_with_no_fixture_under_any_set_keeps_every_file). `cargo clippy --workspace --all-targets -- -D warnings` is clean. `cargo fmt --check` is clean.
    - failure: swissarmyhammer-validators review::tool_rules::tests::shipped::missing_docs::the_shipped_dart_missing_docs_tool_rule_reports_every_fail_fixture_line. The assertion at crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:434 expects 5 findings. The test got 10. Each undocumented Dart member line has two findings (for example "class UndocumentedClass {" two times). This can be a duplicate of diagnostics that the changed LSP or diagnostics code sends two times.
    - next: Find why each Dart diagnostic occurs two times. Make the count 5. Then run the failed test one time.
  timestamp: 2026-10-17T10:09:28.053277+00:00
- actor: claude-code
  id: 01m54nfn6tdhqpcpyar2tc3tbx
  text: |-
    ### test — red is not caused by this task
    - The one failing test, `the_shipped_dart_missing_docs_tool_rule_reports_every_fail_fixture_line`, passes when it runs alone on the same tree (1/1).
    - `swissarmyhammer-validators` does not depend on `swissarmyhammer-lsp` or `swissarmyhammer-diagnostics`. This task changes no Dart code.
    - The other 5225 tests pass. Clippy and fmt are clean.
    - The flaky test is recorded in its own `#test-failure` task. The loop continues to commit.
  timestamp: 2026-10-17T10:10:55.322567+00:00
depends_on:
- 01M53GC28P974P34HEP7XN651M
position_column: doing
position_ordinal: '8480'
title: Do not send ignored paths to the LSP
---
## Problem
The LSP worker (`crates/swissarmyhammer-code-context/src/lsp_worker.rs`) reads rows with `lsp_indexed = 0`. Then it sends `didOpen`, `documentSymbol` and call-hierarchy requests for each file. It does not check the path. When sourcekit-lsp opens a file in `.build/checkouts/<pkg>/`, it finds `<pkg>/Package.swift`. Then it makes a new workspace and starts a full background index for that package. The diagnostics watcher and `LspSession::sync_open` (`session.rs`) also do not check the path.

## Fix
- Put a guard at the single point where sah opens a document in the LSP (`LspSession::sync_open` or the call that it uses). If the shared ignore filter skips the path, or the path is outside the workspace root, do not send the request. Return a clear result (for example, skipped) and do not return an error.
- In the LSP worker, mark skipped rows so that the worker does not read them again in a loop.

## Acceptance criteria
- For an ignored path or a path outside the root, sah sends no `didOpen` and no other LSP request.
- The worker does not read the same skipped rows again and again.

## Tests
- A unit test with a fake LSP client that records the requests. Give it one ignored path and one normal path. Only the normal path gets requests. #lsp-fix #bug