---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvbgxcy8s4jq80qt6z5j8b9g
  text: '[finish] Picked up as first ready task in $diagnostics (ordinal a380). Review column empty. Starting /implement → /test → /review loop.'
  timestamp: 2026-06-17T19:28:45.512187+00:00
- actor: claude-code
  id: 01kvbjtsx5rbwxd75narn55aeq
  text: '[finish] Implementation landed in review. Files changed: swissarmyhammer-lsp/src/session.rs (added is_running/with_client/sync_open), code-context layered_context.rs (now holds SharedLspSession, routes all LSP through session), ops/get_diagnostics.rs (delegates to unified lsp_diagnostics path), lsp_worker.rs (consumes session), lib.rs export, several ops test modules migrated, and swissarmyhammer-tools server.rs + code_context/mod.rs production wiring now passes daemon.session(). Tests: code-context 627+26+2 green/0 warnings; lsp 213+1+1 green; workspace build clean. Adversarial double-check REVISE (worker re-index staleness + diagnostics source_layer shift) — both fixed & re-verified. Next: /test workspace then /review.'
  timestamp: 2026-06-17T20:02:17.637203+00:00
- actor: claude-code
  id: 01kvbpn10xtg9btdrkbdk4zj14
  text: '[finish] Review verdict (iter 1): FINDINGS — 0 blockers, 13 warnings, 4 nits (engine flagged INCOMPLETE: 5/60 tasks failed). Two substantive (error-suppression) warnings: server.rs:469 unwrap_or_default swallows JoinError; layered_context.rs:255 sync_document swallows fs read failure into empty content. Rest: missing doc on has_live_lsp, nesting in parse_hover_contents, extension_to_language_id should be a lookup table, test-fixture dedup (none_session/mock_lsp_session/spawn_mock_lsp), and magic-constant nits. Dispatching /implement to work the checklist (iter 2).'
  timestamp: 2026-06-17T21:09:02.621945+00:00
- actor: claude-code
  id: 01kvbpp288hb1x48bgy6ceksqb
  text: '[implement] Iteration 2 picked up. Moved to doing. Working the Review Findings checklist: 2 substantive (server.rs JoinError swallow, sync_document fs-read swallow), warnings (doc comment, parse_hover helper extraction, extension_to_language_id lookup table, test-fixture dedup), and nits (named constants).'
  timestamp: 2026-06-17T21:09:36.648841+00:00
- actor: claude-code
  id: 01kvjmjpd061eapqbj1kr1kbmk
  text: |-
    Routed from a review run on ^9nj62gm (the review engine swept the shared working tree and flagged uncommitted changes in code-context watcher.rs files, which belong to this task's stream, not ^9nj62gm). Please address or split these — they are real defects, not nits to ignore:

    - BLOCKER — crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs:166 — test helpers `get_ts_indexed` / `get_lsp_indexed` are near-verbatim duplicates differing only in the queried column ('ts_indexed' vs 'lsp_indexed'). Collapse into one column-parameterized helper `get_indexed_flag(conn, path, column)`.
    - WARNING — watcher.rs:91 — debounce loop nests >4 levels (while→match→Ok→for→for→if); extract `process_debounced_events(...) -> Vec<FileEvent>`.
    - NIT — watcher.rs:105 — magic `1` debounce timeout; promote to a named `WATCHER_DEBOUNCE_SECS` constant.

    (Also a near-identical watcher at crates/swissarmyhammer-code-context/src/watcher.rs is dirty in the tree — check whether the same dedup applies there.)
  timestamp: 2026-06-20T13:47:30.080218+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd280
project: diagnostics
title: Rewire code-context onto the shared LSP session; collapse its get_diagnostics
---
## What
Make code-context a pure *consumer* of the shared session, completing the unification. Today code-context spawns/owns the client lifecycle (`lsp_server.rs`, `lsp_worker.rs`, `lsp_indexer.rs`) and `LayeredContext` opens/closes documents per request. After this task it holds a session handle and never opens documents itself.

- `LayeredContext` (`crates/swissarmyhammer-code-context/src/layered_context.rs`) takes an `LspSession` handle from `swissarmyhammer-lsp` instead of a raw `SharedLspClient`. Replace `lsp_request`/`lsp_request_with_document`/`lsp_multi_request_with_document`/`lsp_notify` internals to route through the session (documents are opened/synced by the session/watcher, not per-request).
- `lsp_worker.rs`/`lsp_indexer.rs`: the indexing worker consumes the shared session for symbol collection; remove duplicate spawn/handshake logic now living in swissarmyhammer-lsp.
- **Collapse `ops/get_diagnostics.rs`**: instead of its own pull request, it delegates to the unified diagnostics path (read latest-per-uri from the session cache / trigger a pull through the session). Keep the op's public result shape (`DiagnosticsResult`) stable for the existing `code_context` MCP tool.
- All existing code-context ops (get_definition/get_references/get_hover/get_implementations/get_inbound_calls/etc.) keep working through the session.

## Depends on
- "Add single owned LSP session with shared open-document set in swissarmyhammer-lsp"
- "Capture publishDiagnostics and add in-process fan-out in swissarmyhammer-lsp"

## Acceptance Criteria
- [ ] `LayeredContext` issues all live LSP work through the shared `LspSession`; code-context no longer opens/closes documents or spawns a client.
- [ ] `ops/get_diagnostics.rs` reads from the unified session path; duplicate diagnostics implementation removed.
- [ ] The `code_context` MCP tool's `get diagnostics`/`get definition`/etc. behave unchanged from the caller's view.

## Tests
- [ ] `cargo test -p swissarmyhammer-code-context` — all existing LSP-layer and ops tests pass against the session-backed `LayeredContext` (adapt fixtures to inject a mock session).
- [ ] Integration (gated on rust-analyzer): drive `get diagnostics` on a fixture crate with a known error via the unified path; assert the same report the old path produced.

## Workflow
- Use `/tdd`. Land after the session + fan-out exist so code-context has something to consume. #diagnostics

## Review Findings (2026-06-17 15:44)

> ⚠️ 5/60 review tasks failed — results are INCOMPLETE.

### Warnings
- [ ] `crates/swissarmyhammer-code-context/src/layered_context.rs:41` — Public method `has_live_lsp()` lacks a doc comment. All public items require documentation per the Rust guidelines. Add a doc comment: `/// Returns true if the session has a live LSP client available.`.
- [ ] `crates/swissarmyhammer-code-context/src/layered_context.rs:255` — Silent error suppression in `sync_document`: `std::fs::read_to_string().unwrap_or_default()` swallows file-not-found errors and returns empty string. If a file is deleted or inaccessible, the caller receives no signal and proceeds with stale/empty content, which could silently corrupt LSP state. Propagate the error: `let text = std::fs::read_to_string(file_path)?;` to distinguish real failures from graceful LSP unavailability.
- [ ] `crates/swissarmyhammer-code-context/src/layered_context.rs:838` — Test helper `none_session` is duplicated across test modules instead of defined once and reused. Nearly identical implementation (0.96 similarity) exists in `lsp_worker.rs:565`. Extract `none_session` to a shared test fixture module (e.g., `crate::test_fixtures`) and call it from both layered_context and lsp_worker tests instead of duplicating.
- [ ] `crates/swissarmyhammer-code-context/src/layered_context.rs:2296` — Test helper `mock_lsp_session` is duplicated across test modules. Existing identical implementations (1.00 similarity) are already defined in `get_inbound_calls.rs:1689` and `workspace_symbol_live.rs:716`. Adding it here creates a third copy instead of reusing. Extract `mock_lsp_session` to a shared test fixture module and import it from all test modules instead of maintaining multiple identical copies.
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:525` — The `extension_to_language_id` match arms over file extensions are each a constant string mapping; this should be expressed as a lookup table (static map or phf), not as parallel match arms a human must keep in lockstep. Convert to a static `&[(extensions, language_id)]` table or lazy-loaded `HashMap`, then perform a linear/binary search or use `phf::Map` for O(1) lookup. The function becomes a single code path interpreting table data instead of N parallel arms.
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:565` — Test helper `none_session` is duplicated across test modules instead of defined once and reused. Nearly identical implementation (0.96 similarity) exists in `layered_context.rs:838`. Extract `none_session` to a shared test fixture module (e.g., `crate::test_fixtures`) and call it from both lsp_worker and layered_context tests instead of duplicating.
- [ ] `crates/swissarmyhammer-code-context/src/ops/get_code_actions.rs:1308` — Test helper `mock_lsp_session` is duplicated across test modules. Existing identical implementations (1.00 similarity) are already defined in `get_inbound_calls.rs:1689` and `workspace_symbol_live.rs:716`. Adding it here creates a third copy instead of reusing. Extract `mock_lsp_session` to a shared test fixture module and import it from all test modules instead of maintaining multiple identical copies.
- [ ] `crates/swissarmyhammer-code-context/src/ops/get_diagnostics.rs:537` — spawn_mock_lsp reimplements an existing test helper already defined in at least 4 other files (layered_context.rs, workspace_symbol_live.rs, get_inbound_calls.rs, get_code_actions.rs). Per probe, 0.99 similarity — effectively identical implementations. Each copy must be updated independently when the mock protocol changes, creating maintenance burden. Extract spawn_mock_lsp to a shared test fixture module (e.g., test_fixtures.rs or a new test_lsp_helpers module) and reuse it across all files. Reduces duplication and ensures protocol changes apply uniformly.
- [ ] `crates/swissarmyhammer-code-context/src/ops/get_diagnostics.rs:556` — mock_session_and_file reimplements an existing helper already defined in workspace_symbol_live.rs, get_inbound_calls.rs, get_code_actions.rs, and layered_context.rs (there named mock_lsp_session). Per probe, 0.93 similarity — same pattern of spawning mock LSP and creating a session. Return signature differs slightly (this version also returns the TempDir), but that's a minor variation that a generalized shared helper could accommodate. Unify into the same test fixture module as spawn_mock_lsp. Parameterize the return type or have the shared version return both (session, temp_dir) so callers get what they need — simpler than maintaining parallel implementations.
- [ ] `crates/swissarmyhammer-code-context/src/ops/get_hover.rs:129` — parse_hover_contents has 4+ levels of nesting within the array-parsing case. The filter_map closure contains nested if-let statements (checking for string vs. {language, value} object) combined with conditional formatting logic, making the control flow difficult to follow. The function handles three distinct LSP response formats (MarkupContent, MarkedString, array), each with separate parsing paths, which compounds the complexity. Extract array parsing into a helper function like `parse_marked_string_array(arr: &[Value]) -> Vec<String>` to reduce nesting in the main function. This separates concerns: the top level dispatches on response shape, and the helper handles the complex iteration logic in isolation.
- [ ] `crates/swissarmyhammer-code-context/src/ops/get_inbound_calls.rs:1689` — mock_lsp_session is reimplemented identically across multiple test modules instead of reusing a shared test utility. Four files define the same function with 0.99–1.00 similarity: get_code_actions.rs:1307, workspace_symbol_live.rs:716, layered_context.rs:2295. Test fixture duplication obscures a single canonical setup path and makes maintenance harder when the LSP mocking contract changes. Extract mock_lsp_session to crate::test_fixtures module and import it in all four test modules (get_inbound_calls, get_code_actions, workspace_symbol_live, layered_context). This keeps the setup contract in one place and reduces test maintenance burden.
- [ ] `crates/swissarmyhammer-code-context/src/ops/workspace_symbol_live.rs:716` — mock_lsp_session is reimplemented identically instead of importing from test_fixtures (or wherever the canonical version lives). Same issue as get_inbound_calls.rs:1689. Extract mock_lsp_session to crate::test_fixtures and import it here (and in all other test modules that define it).
- [ ] `crates/swissarmyhammer-tools/src/mcp/server.rs:469` — `await` on a `JoinHandle` that panicked returns `Err(JoinError)`, but `.unwrap_or_default()` silently converts it to an empty `Vec`. The panic is lost with no logging, masking bugs in the spawned task. Either log the error explicitly before converting: `let clients = lsp_handle.await.map_err(|e| { tracing::error!("LSP supervisor panic: {}", e); e }).unwrap_or_default();` or propagate the error up and let the caller decide, or return `Result<Vec<...>>` instead.

### Nits
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:80` — Hardcoded batch size `10` configures the LSP indexing worker's batch size in tests but appears as a bare literal. This value is repeated across multiple test configurations without a named constant. Extract a test constant: `const TEST_BATCH_SIZE: usize = 10;` at the top of the test module and use it in all LspWorkerConfig creations.
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:129` — Hardcoded sleep duration `Duration::from_millis(1)` configures test behavior but lacks a named constant. This duration and other test timeouts (5ms, 20ms, 30ms, 50ms) are scattered throughout the test suite, making them hard to adjust consistently. Extract test timeout constants at the top of the test module: `const TEST_SLEEP_MINIMAL: Duration = Duration::from_millis(1); const TEST_SLEEP_SHORT: Duration = Duration::from_millis(5);` etc., then use them throughout tests.
- [ ] `crates/swissarmyhammer-code-context/src/lsp_worker.rs:458` — Hardcoded file size `1024` in the test fixture setup lacks a named constant. While test data, it configures the row being inserted into indexed_files and should be explicit about what it represents. Extract a constant: `const TEST_FILE_SIZE: u32 = 1024;` and use it in test fixture setup to clarify it represents a synthetic test file size.
- [ ] `crates/swissarmyhammer-code-context/src/ops/get_implementations.rs:51` — Hardcoded limit of 20 for default max_results should be a named constant — it configures the maximum number of implementation results returned when no explicit limit is provided. Define a named constant (e.g., `const DEFAULT_MAX_IMPLEMENTATIONS: usize = 20;`) at module level and use it here instead of the bare `20`.