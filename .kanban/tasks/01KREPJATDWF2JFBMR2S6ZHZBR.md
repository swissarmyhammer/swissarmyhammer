---
assignees:
- claude-code
depends_on:
- 01KREM6B7X01T8WXDA258DS2K9
position_column: done
position_ordinal: ffffffffffffffffffffffffbe80
project: semantic-search
title: Add doctor smoke check that exercises semantic search
---
## What

Add a smoke check to `code-context doctor` that exercises the actual user-facing semantic-search behavior, not just the presence of config/files. The check opens the current workspace's index, runs a tiny canary query through `search_code`, and reports whether it returned anything.

The `doctor` command exists today (`code-context-cli/src/commands/doctor.rs` per the call in `main.rs:84`) but the checks it runs validate config and file presence — they would have happily passed today while semantic search was returning zero results. We need probes that hit the actual capability path.

### Why this card exists

This is the third line of defense (after card 1's status surfacing and card 4's e2e test). The status check catches the failure in CI-style settings; the doctor check catches it in user-facing settings, where a developer types `code-context doctor` to figure out why semantic search isn't working. Today `doctor` does not exist as a diagnostic for *that* problem.

### Files

- `code-context-cli/src/commands/doctor.rs` (or wherever `run_doctor` lives — verify with `grep -rn 'fn run_doctor' code-context-cli/`). Add a new check function `check_semantic_search_smoke`.
- Possibly `swissarmyhammer-code-context/src/doctor.rs` or similar if there's a layered doctor abstraction in the code-context crate. Check first.
- Reuse `Embedder::default()` and `search_code()` directly — no new dependencies.

### Approach

1. Add a new doctor check `check_semantic_search`:
   - Open the current workspace's index (read-only).
   - Run `SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL`. If 0, report: `❌ Semantic search index is empty — no chunks have embeddings. Run indexing.`
   - If > 0: construct `Embedder::default()`, embed the canary query `"function that handles errors"`, call `search_code(&conn, embed, &SearchCodeOptions { top_k: 1, min_similarity: 0.0, .. })`.
   - Assert the result has at least one match. If empty, report: `❌ Semantic search returned no results for canary query — embedding model dimension may not match stored embeddings.`
   - If match returned: report: `✓ Semantic search functional (canary returned X matches).`
2. The check should be FAST. The embedder load is the expensive step (~1-3s); subsequent embeds are sub-second. Cache the embedder for the life of the doctor run if multiple checks use it.
3. Match the existing reporter API used by other doctor checks. Look at `code-context-cli/src/commands/doctor.rs` to see the existing check structure; copy that pattern.
4. The check should not fail the doctor exit code unless `--strict` is passed; report warnings by default so a user with a partially-indexed workspace doesn't get a noisy red exit. (Verify the existing `verbose` / strictness behavior of doctor.)

### Scope discipline

- Only add the semantic-search probe in this card. Other "advertised capability" probes (callgraph, blastradius, find_duplicates) belong in follow-up cards filed by card A.
- Do NOT touch the existing init/deinit/skill flows.

## Acceptance Criteria

- [x] `code-context doctor --verbose` runs the new check and prints one of: `✓ functional`, `⚠ empty index`, `❌ canary returned no results`.
- [x] Run against THIS workspace today (before card 2): the check correctly reports `❌ no chunks have embeddings`.
- [x] Run against a workspace after card 2 has indexed: the check correctly reports `✓ functional`.
- [x] The check does not crash doctor when the workspace has no `.code-context/` directory yet — it reports that condition cleanly.
- [x] No new top-level dependencies; reuse `swissarmyhammer-embedding` and `swissarmyhammer-code-context` which the CLI already imports.

## Tests

- [x] `code-context-cli/src/commands/doctor.rs` — unit test: stub a workspace with 0 embeddings; assert the check reports `empty index`.
- [x] `code-context-cli/src/commands/doctor.rs` — unit test: stub a workspace with at least one embedded chunk that the canary query should hit; assert the check reports `functional`. May need a small `Embedder` fixture or skip the actual embed if model not available in CI — match the pattern used by other tests that gate on `Embedder::default()`.
- [x] Run: `cargo nextest run -p code-context-cli` — all pass.
- [x] Manual: run `./target/debug/code-context doctor --verbose` on this workspace before and after card 2 lands; output should change appropriately.

## Workflow

- Use `/tdd` — write the failing checks first. #code-context

## Review Findings (2026-05-12 22:45)

### Warnings
- [x] `code-context-cli/src/commands/doctor.rs:357-390` — The Error branch claiming "query embedding dimension likely does not match the stored embeddings" is effectively unreachable, which means the doctor will produce a **false-positive `Ok`** in the exact dimension-mismatch case it is meant to catch. `model_embedding::cosine_similarity` returns `0.0` when the two vector lengths differ (see `model-embedding/src/similarity.rs:12-14`), and the `search_code` filter `*sim >= options.min_similarity` is inclusive (`search_code.rs:129`). With `min_similarity: 0.0`, a sim of `0.0` from a dim-mismatched chunk still passes the filter and shows up as a match, so `result.matches.is_empty()` will be false even when the embedder dimension does not match what is stored. The doctor will then report "Semantic search functional (canary returned 1 match …)" while semantic search is in fact broken. The implementer documented this trade-off in `check_semantic_search_with_embedded_chunks_is_functional_or_warns` ("If dimensions disagree, `search_code` returns sim=0 — but we use min_similarity=0.0 in the check, so any match counts"), but accepting that trade-off defeats the third-line-of-defense purpose stated in the card. Suggested fix: use a tiny positive threshold (e.g. `min_similarity: f32::EPSILON`) so dim-mismatched chunks (sim=0.0) fall out of the result set, or inspect the top match's `similarity` after `search_code` returns and surface an Error when it is `<= 0.0`. Add a unit test that inserts a chunk whose embedding dimension intentionally differs from `Embedder::default()`'s and asserts the check produces `Error` (or at minimum `Warning`), not `Ok`.

  **Fix applied (2026-05-12, iteration 2)**: Switched the canary `SearchCodeOptions.min_similarity` from `0.0` to `f32::EPSILON` so dimension-mismatched chunks (sim=0.0) are filtered out and the Error branch fires. The Ok-branch doc-comment and the Error-branch message were updated to reflect the new threshold. Replaced the trade-off test (`check_semantic_search_with_embedded_chunks_is_functional_or_warns`) with two sharper tests: `check_semantic_search_dimension_mismatch_is_not_ok` (asserts a 3-dim stored embedding never produces Ok — regression guard for this finding) and `check_semantic_search_with_matching_dimension_is_functional` (loads the real embedder, embeds a probe to learn the runtime dimension, inserts a same-dim chunk, asserts Ok).

### Nits
- [x] `code-context-cli/src/commands/doctor.rs:431-438` — `make_empty_index` returns the rw `Connection` and the empty-index test binds it as `_conn`, holding it open while `check_semantic_search` reopens the same DB read-only. SQLite tolerates this, but the convention used in `swissarmyhammer-code-context` tests is to drop the rw handle before reopening (the third test does drop it explicitly). For consistency and to avoid surprising other readers, either drop the rw connection at the end of `make_empty_index` (after `create_schema`) or have `check_semantic_search_empty_index_warns` drop it before calling the check.

  **Fix applied**: `check_semantic_search_empty_index_warns` now binds the connection as `conn` and explicitly `drop(conn)` before invoking the check. `make_empty_index` still returns the rw connection because the functional test path needs to populate chunks through it.

- [x] `code-context-cli/src/commands/doctor.rs:309-341` — The `Embedder::default()` construction error and the `embedder.load()` error are nearly identical Warning branches with the same fix hint shape. Consider folding them into a single helper that returns `Result&lt;Embedder, Check&gt;` so the doctor path reads top-to-bottom without two near-duplicate early returns. Not load-bearing; the current shape is readable.

  **Fix applied**: Extracted `load_default_embedder(embedded_count: i64) -> Result<Embedder, Check>` adjacent to `check_semantic_search`. Both warning branches now live in the helper; the caller is one `match` expression.

- [x] `code-context-cli/src/commands/doctor.rs:1-8` — The module-level doc-comment lists the checks but does not mention LSP probes (covered by `check_lsp_status`). Either add `LSP server availability per detected project type` to the bullet list or drop the list and let the function docs speak for themselves. The block already lists "LSP server availability per detected project type" — disregard if I misread; double-check on next iteration.

  **No action needed**: The doc-comment already includes `LSP server availability per detected project type` (line 7). Reviewer flagged this as "disregard if I misread" — confirmed: nothing to fix.