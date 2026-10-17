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
- actor: claude-code
  id: 01m54p3vfdvkcne4aygga7y0kt
  text: |-
    ### review — findings
    - evidence: review sha HEAD~1..HEAD (commit 0df7c1581); 3 findings (3 confirmed, 1 refuted) — crates/swissarmyhammer-code-context/src/layered_context.rs:632, crates/swissarmyhammer-code-context/src/lsp_worker.rs:276 (duplication/duplication), crates/swissarmyhammer-code-context/src/lsp_worker.rs:276 (reuse/reuse)
    - next: implement the 3 open findings in the dated Review Findings section. Then run test, commit and review again. The two lsp_worker.rs findings name the same cause. The duplication finding says not to change `mark_failed_file_indexed` in this change. The reuse finding says that both functions can call the shared helper.
  timestamp: 2026-10-17T10:21:57.101623+00:00
- actor: claude-code
  id: 01m54p4sfqbyes78rmfmgt3ad0
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 15 files (LspSession path filter seam, DocumentSync::Skipped, worker, diagnostics, leader route)
    - test: red — 5225 of 5226; the 1 failure is a flaky Dart validator test that passes alone (own #test-failure task)
    - commit: 0df7c1581
    - review: findings — crates/swissarmyhammer-code-context/src/layered_context.rs:632, crates/swissarmyhammer-code-context/src/lsp_worker.rs:276, crates/swissarmyhammer-code-context/src/lsp_worker.rs:276
  timestamp: 2026-10-17T10:22:27.831968+00:00
- actor: claude-code
  id: 01m54p8tqg2hm0qtgx8j2m1rdd
  text: |-
    Review findings fixed (TDD, 3 new tests seen RED first, then GREEN).
    - layered_context.rs `sync_document`: after the `is_running` check, it calls the existing `LspSession::skips_path` (the same filter that `sync_open` uses, one rule set) and returns Ok(None) before the file read. New tests: a filtered missing path and a filtered non-UTF-8 path both give Ok(None). Before the fix they gave Io NotFound and Io InvalidData.
    - lsp_worker.rs `index_single_file` had the same cause (read before the filter). It now checks `skips_path` before the read and returns Ok(Skipped). New test: a filtered missing file gives Ok(Skipped) (before: Io NotFound).
    - lsp_worker.rs: new `mark_file_indexed_or_warn(db, relative_path, context)`. Both `mark_failed_file_indexed` and `mark_skipped_file_indexed` call it. The `debug!` line stays in the skipped function. The duplication finding says not to change `mark_failed_file_indexed`; the reuse finding and the dispatcher order say to have both call the helper. The dispatcher order was followed. Only the warning text changed ("as lsp_indexed after error" / "as lsp_indexed after skip").
  timestamp: 2026-10-17T10:24:40.176313+00:00
- actor: claude-code
  id: 01m54p8xtk76zs32a1tfhzy2v4
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-code-context/src/layered_context.rs, crates/swissarmyhammer-code-context/src/lsp_worker.rs. `cargo nextest run -p swissarmyhammer-code-context`: 735 passed, 0 failed. `cargo clippy -p swissarmyhammer-code-context --all-targets -- -D warnings`: clean. `cargo fmt --all --check`: clean.
    - next: /test, then /review
  timestamp: 2026-10-17T10:24:43.347373+00:00
- actor: claude-code
  id: 01m54pjycdgpa5ybfv1ggfa89g
  text: |-
    ### test — green (iteration 2)
    - evidence: `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-code-context)'` ran 4996 tests. 4996 passed, 0 failed, 0 skipped (62 slow). `cargo clippy --workspace --all-targets -- -D warnings` shows no warnings (exit 0). `cargo fmt --check` shows no differences (exit 0).
    - next: Do the review step.
  timestamp: 2026-10-17T10:30:11.597748+00:00
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

## Review Findings (2026-10-17 05:11)

> Scope: `review sha HEAD~1..HEAD` — reviewed the diffs only — lines this change added or modified. 14 file(s) reviewed, 7 not reviewed.

> 6 file(s) not reviewed — excluded by an ignore rule:
> - `.kanban/ (from .reviewignore)` — 6 file(s)

> 1 file(s) not reviewed — no validator matched:
> - `ARCHITECTURE.md` — no validator matches this file

- [x] `crates/swissarmyhammer-code-context/src/layered_context.rs:632` `completeness/invariant-propagation` — The path filter is checked only inside session.sync_open, after sync_document has already read the file. For a filtered path that cannot be read as UTF-8, or that was deleted, read_to_string fails first and the error propagates. The doc comment says a skipped path gives Ok(None) with no request sent, so this contradicts the contract. Ignored build output such as .build/ or target/ can hold binary files, so this is reachable in practice. Check the path filter before reading the file, the same way the is_running check short-circuits. Either expose a session method that asks the filter, or return Ok(None) early when the path is filtered. Add a test where a filtered path holds non-UTF-8 bytes or does not exist, and assert Ok(None).
- [x] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:276` `duplication/duplication` — The new `mark_skipped_file_indexed` repeats the body of the existing `mark_failed_file_indexed`. Both lock the DB with the same poison recovery, call `mark_lsp_indexed` with the same arguments, and log a warning when the mark fails. Only the log wording differs. The mark-and-warn logic can now drift between the two copies, for example if the poison handling or the mark step changes in one function only. Extract one shared helper that takes the message as a parameter, for example `fn mark_indexed_or_warn(db: &SharedDb, relative_path: &str, context: &str)`, and call it from `mark_skipped_file_indexed` in the changed code. Keep the `debug!` line in the new function. Because `mark_failed_file_indexed` is outside this change, switching it to the helper is a separate follow-up task and should not be done in this change.
- [x] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:276` `reuse/reuse` — `mark_skipped_file_indexed` repeats the body of `mark_failed_file_indexed` (lock the DB, call `mark_lsp_indexed`, log a warning on failure) and differs only in its log messages. Two parallel copies of the same mark-and-log step will drift apart, so a fix to one path will be missed in the other. Extract one helper, for example `mark_file_indexed(db, relative_path, context: &str)`, that takes the log context. Have both `mark_failed_file_indexed` and `mark_skipped_file_indexed` call it, or fold the skipped case into the failure helper with a parameter for the message.
