---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzscav5te39kqy9wh2tb4eez
  text: |-
    Research done. Findings:

    - The card's line numbers are correct for every production site: `tool_registry.rs` 409/454/468/479/494/767, `server.rs` 1284/1286/2443/2455, `edit/mod.rs` 1368, `write/mod.rs` 248, `shared_utils.rs` 114/134.
    - `edit/mod.rs:3313` and `shared_utils.rs:2007` and the integration test at `file_tools_integrations.rs:2317` all exercise the RESULT-envelope `mutated_paths`, which the card keeps. They stay green; the stale prose that names the removed side-channel is what changes.
    - The test that MUST go is `edit/mod.rs::cascade_records_mutated_path_on_success` (near line 2788). It calls `with_fresh_mutated_paths` and `take_mutated_paths` directly.
    - `produce_outcome` and `is_diagnosable` stay. The `diagnostics` tool is their other consumer, so removing `inline_diagnostics.rs` leaves no dead symbol behind.
    - `edit/description.md` states "Diagnostics for the edited file are folded into the result when it is a diagnosable source file." That sentence becomes false with the feature gone, so it goes with it.
    - The only place the fold can be observed end to end is the dispatch chokepoint (`McpServer::execute_tool` / `call_tool`), never `tool.execute` directly. A regression test therefore has to go through `execute_tool`.
  timestamp: 2026-08-11T21:40:52.538736+00:00
- actor: claude-code
  id: 01kzsdb748ftm3j6gbsx422qp5
  text: |-
    Implementation landed. What I did, item by item against the card:

    1. Deleted `crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs`.
    2. Removed `pub mod inline_diagnostics;` from `mcp/mod.rs`.
    3. `server.rs`: both chokepoints now call `tool.execute(...)` and return it. `execute_tool` keeps the plain clone of the shared context; `call_tool` keeps `prepare_tool_context(peer)`.
    4. `tool_registry.rs`: dropped the `mutated_paths` field, its initializer, `record_mutated_path`, `take_mutated_paths`, `with_fresh_mutated_paths`, and the isolating clone in `ToolContext::call_tool`.
    5. `edit/mod.rs` and `write/mod.rs`: dropped the `record_mutated_path` call and the comment block above each.
    6. Tests — read this closely, it is not what the card assumed:
       - Deleted `edit/mod.rs::cascade_records_mutated_path_on_success`. This was the one test that drove the removed side-channel.
       - `edit/mod.rs::successful_edit_carries_tagged_content_and_mutated_paths` (card line 3313), the `shared_utils.rs` test at 2007, and `file_tools_integrations.rs:2317` all exercise the RESULT-envelope `mutated_paths`, which the card's "Keep" section keeps. None of them needed a change and all three pass unchanged. I edited only the stale `shared_utils.rs` doc prose above them, which still named `record_mutated_path` and the fold-in convention.
    7. Docs: `ideas/diagnostic.md` and `ideas/file-edit-tools.md` each carry a status note that inline-on-edit was built and withdrawn, and every sentence that stated it as live behavior is corrected. Also corrected `files/edit/description.md`, which told the model "Diagnostics for the edited file are folded into the result" — that sentence was a lie once the fold was gone.

    No new test. I wrote one first (an `execute_tool` edit of a `.rs` file asserting no `diagnostics` / `pending` / `diagnostics_unavailable` key) and removed it, because it PASSED before the removal. The reason is worth recording for the next agent: the fold's own gate means it attaches nothing when the analysis comes back clean, and in a test process the leader answers clean. So the assertion could not distinguish the two states, and whether it ever went red would depend on the ambient LSP leader. A test that cannot fail proves nothing. The removal is compiler-enforced instead: every removed symbol is a build error if referenced.

    Also noted: card ^6xjxebg (done) carries two unchecked review findings asking for a doc comment on the `inline_diagnostics` module. They are now unactionable. That card is closed and its findings were already classified as pre-existing, so I left its history alone rather than rewriting it.
  timestamp: 2026-08-11T21:58:33.352357+00:00
- actor: claude-code
  id: 01kzsdbg1vkkz30sxdm29fxn5v
  text: |-
    ### implement — changed
    - evidence: 10 files — deleted `crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs`; edited `mcp/mod.rs`, `mcp/server.rs`, `mcp/tool_registry.rs`, `mcp/tools/files/edit/mod.rs`, `mcp/tools/files/edit/description.md`, `mcp/tools/files/write/mod.rs`, `mcp/tools/files/shared_utils.rs`, `ideas/diagnostic.md`, `ideas/file-edit-tools.md`. `cargo build --workspace --all-targets` clean, zero warnings. `cargo fmt --all` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean. `cargo nextest run --workspace --no-fail-fast` → 14097 run, 14093 passed, 4 failed. The 4 (`review_e2e` x3, `review_progress_stdio_test` x1) reproduce identically with this change stashed, so they are pre-existing and not from this card. `diagnostics` tool: 29 tests pass.
    - next: /review
  timestamp: 2026-08-11T21:58:42.491309+00:00
position_column: doing
position_ordinal: '8580'
title: Remove inline-on-edit LSP diagnostics from the file mutation path
---
## Problem

Every `files` edit and write blocks on LSP diagnostics before it returns. The
`fold_in_diagnostics` chokepoint runs after each mutating tool call. For one
edited file it does:

- `open_workspace` for blast radius (follower retry backs off up to ~5 s)
- `get_blastradius` one-hop, which adds every dependent file to the work list
- `sync_open` + `pull_diagnostics` **serial, per file and per dependent**, each
  with a 30 s timeout
- a `settle` quiescence wait, minimum 300 ms, hard cap 5 s

The inline path hardcodes `DiagnosticsConfig::default()`, so `include_dependents`
is true. No env var or config turns it off.

The feature intent was fewer review passes. That did not happen. The `diagnostics`
MCP tool already gives explicit, on-demand diagnostics, so the inline path is
redundant.

Note: tree-sitter is NOT in this path. Tree-sitter parsing and embeddings run in
the file watcher, off the request path.

## Task

Remove the inline-on-edit diagnostics feature completely.

1. Delete `crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs`.
2. Remove the module declaration from `crates/swissarmyhammer-tools/src/mcp/mod.rs`.
3. Remove both `fold_in_diagnostics` call sites in
   `crates/swissarmyhammer-tools/src/mcp/server.rs` (lines 1286 and 2455) and the
   paired `with_fresh_mutated_paths` calls (lines 1284 and 2443).
4. Remove the diagnostics side-channel from
   `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs`: the `mutated_paths`
   field (line 409), `record_mutated_path` (468), `take_mutated_paths` (479),
   `with_fresh_mutated_paths` (494), the initializer (454), and the call at 767.
5. Remove the `record_mutated_path` calls in
   `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs:1368` and
   `crates/swissarmyhammer-tools/src/mcp/tools/files/write/mod.rs:248`.
6. Update the affected tests: `edit/mod.rs` test at line 3313,
   `shared_utils.rs` test at 2007, and
   `crates/swissarmyhammer-tools/tests/integration/file_tools_integrations.rs`.
7. Update the docs in `ideas/diagnostic.md` and `ideas/file-edit-tools.md`.

## Keep

- The `diagnostics` MCP tool and the whole `swissarmyhammer-diagnostics` crate.
  They stay as the on-demand path.
- The `mutated_paths` field in the tool RESULT envelope
  (`files/shared_utils.rs:114,134`). It comes from the function arguments, not
  from the context side-channel. It is independent of this removal.
- The file watcher and code-context indexing. They are not on the request path.

## Done when

- No reference to `inline_diagnostics`, `fold_in_diagnostics`,
  `record_mutated_path`, `take_mutated_paths`, or `with_fresh_mutated_paths`
  is left in the workspace.
- `cargo build` and the full test suite are green with no warnings.
- An edit of a Rust file returns with no LSP round-trip.
- The `diagnostics` MCP tool still works.