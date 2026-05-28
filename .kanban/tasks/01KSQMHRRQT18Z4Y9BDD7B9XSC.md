---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc380
title: code_context_mcp_e2e rebuild/detect tests time out under parallel nextest
---
## DONE (2026-05-28)

Same root cause as the existing `treesitter-embedding` nextest group (documented in `.config/nextest.toml`): these are real e2e tests that each load the qwen-embedding model (and run LSP indexing / file watching). nextest isolates each test in its own process (no in-process model cache), so running them in parallel triggers N cold model loads that contend catastrophically — the rebuild/detect/qwen tests then hang until the 300s slow-timeout kills them. All pass under `--test-threads=1`.

The repo already fixes this for the in-crate unit tests via a `max-threads=1` test-group, but its filter (`test(/^mcp::tools::code_context::tests::/)`) only matches the unit module — NOT these tests, which live in two separate integration binaries (`code_context_mcp_e2e_test` and the `tools_tests` `qwen_embedding_*` / `semantic_search` tests).

Fix: added a `[[profile.default.overrides]]` routing those binaries' tests into the existing `treesitter-embedding` (max-threads=1) group:
`filter = "package(swissarmyhammer-tools) and (binary(code_context_mcp_e2e_test) or test(/qwen_embedding/) or test(/semantic_search/))"`.

This MUST be a nextest test-group, not `#[serial]`: the tests span two test binaries (two processes), and `#[serial]` only serializes within a single process, whereas a test-group caps concurrency across the entire run. It's a resource/concurrency limit, not a timing hack.

Verification: `cargo nextest run -E '<that filter>' --test-threads=8` → **11 tests, 11 passed in 144s** (the group forced serial execution despite -j8). Previously this set hung at 300s under parallelism. One test (`qwen_embedding_find_duplicates_e2e`, 116s) is legitimately heavy (lots of embeddings + O(n²) similarity) but well under the group's 360s slow-timeout — it only *hung* before because of contention, not its own cost.

Acceptance criteria:
- [x] Identified the shared LSP/index/model-load contention as the cause (matches the existing treesitter-embedding pattern), not a per-test bug.
- [x] Serialized the e2e set via a nextest test-group (cross-binary), reusing the established mechanism — no `#[serial]` (wrong scope) and no global `--test-threads=1`.
- [x] All 11 affected tests pass under requested 8-way parallelism (group serializes them); no 300s hang.