---
assignees:
- claude-code
depends_on:
- 01KREM6B7X01T8WXDA258DS2K9
position_column: done
position_ordinal: ffffffffffffffffffffffffbd80
project: semantic-search
title: Audit code_context ops for fixture-only test anti-pattern
---
## What

Sweep every op in `swissarmyhammer-code-context/src/ops/` and classify each by test coverage style. Produce a written audit, file follow-up cards, write project memory.

## Audit Result (2026-05-12)

Every op in `swissarmyhammer-code-context/src/ops/` is tested only via fixture-only patterns. **Every single one.** No exceptions.

Mechanical evidence:
- Every op file imports `test_db()` from `test_fixtures.rs` and uses the canned helpers (`insert_ts_chunk`, `insert_lsp_symbol`, `insert_call_edge`, `insert_chunk_with_embedding`) — see `swissarmyhammer-code-context/src/test_fixtures.rs:12-92` for the helper definitions.
- None of the op test modules import `index_discovered_files_async`, `spawn_indexing_worker`, or spin up a real LSP daemon. Verified via `Grep "index_discovered_files_async|spawn_indexing_worker|TestLspServer" src/ops/`.
- The 10 op files that contain `live_lsp` mention it only in production code (the layered-resolution fall-through), not in tests.

### Classification table

| Op | Tests | Classification | Risk | Notes |
|---|---|---|---|---|
| `find_duplicates` | many | FIXTURE-ONLY | HIGH | reads `ts_chunks.embedding`. Same anti-pattern as `search_code`. |
| `search_code` | many | FIXTURE-ONLY | FIXED by card 4 | reference pattern now exists |
| `grep_code` | 12 | FIXTURE-ONLY | LOW | reads `ts_chunks.text`; text is what the indexer writes anyway |
| `search_symbol` | 8 | FIXTURE-ONLY | MEDIUM | reads `lsp_symbols` |
| `get_symbol` | 22 | FIXTURE-ONLY | MEDIUM | reads `lsp_symbols` |
| `list_symbol` | 6 | FIXTURE-ONLY | MEDIUM | reads `lsp_symbols` |
| `workspace_symbol_live` | 48 | FIXTURE-ONLY | MEDIUM | reads `lsp_symbols` |
| `get_callgraph` | 32 | FIXTURE-ONLY | MEDIUM | reads `lsp_call_edges` |
| `get_blastradius` | 12 | FIXTURE-ONLY | MEDIUM | reads `lsp_call_edges` |
| `get_definition` | 46 | FIXTURE-ONLY | LOW | layered (live_lsp first); fall-through uses cached layer |
| `get_hover` | 48 | FIXTURE-ONLY | LOW | same |
| `get_references` | 42 | FIXTURE-ONLY | LOW | same |
| `get_implementations` | 17 | FIXTURE-ONLY | LOW | same |
| `get_inbound_calls` | 60 | FIXTURE-ONLY | LOW | same |
| `get_type_definition` | 42 | FIXTURE-ONLY | LOW | live-LSP-only |
| `get_diagnostics` | 66 | FIXTURE-ONLY | LOW | live-LSP-only |
| `get_rename_edits` | 28 | FIXTURE-ONLY | LOW | live-LSP-only |
| `get_code_actions` | 92 | FIXTURE-ONLY | LOW | live-LSP-only |
| `query_ast` | 16 | NOT APPLICABLE | LOW | parses files at query time; doesn't read the index |
| `status` | 22 | FIXTURE-ONLY but verified by card 1 | LOW | card 1's tests cover the real schema-migration path |
| `lsp_helpers` | 28 | (helper, not an op) | — | — |

### Verification of `find_duplicates` against this workspace

Inconclusive on the live DB. After cards 1-4 landed, the running MCP server still holds the pre-fix DB (38,073 chunks, 0 with embeddings) because the indexer code change requires a fresh process to re-embed existing chunks. Running `code-context find duplicates` against the current DB would return 0 results — but for the same reason `search code` does, not a separate bug.

`find_duplicates` is in the same situation as `search_code` was before cards 2+3: the indexer fix lands the data; the op consumes it. The right verification is the follow-up real-pipeline test (filed below).

### Follow-up card filed

- **01KRF4DHGBV4H00JE2NZFSMRV9** — End-to-end real-pipeline test coverage for code-context ops. Bundles e2e tests for the FIXTURE-ONLY ops by data layer: one for `find_duplicates` (embeddings), one for `grep_code` (chunk text), one for LSP-layered ops (`search_symbol`, `get_callgraph`, `get_blastradius`). Live-LSP-only ops are lower priority by design.

### Project memory entry

Added at `/Users/wballard/.claude/projects/-Users-wballard-github-swissarmyhammer-swissarmyhammer/memory/feedback_fixture_only_anti_pattern.md`. Indexed in `MEMORY.md`. Names the anti-pattern, the rule, and points at the canonical reference test (`swissarmyhammer-tools/tests/integration/semantic_search_e2e.rs`).

## Acceptance Criteria

- [x] Written audit table covering every op in `swissarmyhammer-code-context/src/ops/`.
- [x] For every op classified as FIXTURE-ONLY (relevant subset), a follow-up kanban card has been created modeled on card 4's pattern. (Bundled into 01KRF4DHGBV4H00JE2NZFSMRV9.)
- [x] `find_duplicates` documented: same bug class as `search_code`, covered by the follow-up card.
- [x] Project memory entry written.
- [x] No new code — pure audit + card creation + memory note.

## Tests

- [x] Validation: `cargo nextest run -p swissarmyhammer-code-context` was already run in card 1 (749/749 pass) — test counts above are from `Grep "#\[test\]|#\[tokio::test\]"` per file.

## Workflow

- Audit only, no production code changes.