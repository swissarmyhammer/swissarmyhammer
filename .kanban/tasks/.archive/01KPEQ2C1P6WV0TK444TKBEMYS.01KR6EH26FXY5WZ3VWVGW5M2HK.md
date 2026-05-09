---
assignees: []
position_column: todo
position_ordinal: '9280'
title: Fix code-context workspace_e2e_test::test_lsp_reindexing_after_file_change (LSP re-index never fires)
---
`swissarmyhammer-code-context/tests/workspace_e2e_test.rs::test_lsp_reindexing_after_file_change` fails after ~60 seconds during `cargo nextest run --workspace`. The test writes a Rust file, verifies the initial LSP index has 9 symbols for `src/lib.rs`, then modifies the file and polls for re-index. Output shows the initial index succeeded but the re-index polling loop prints `Poll re-index LSP: 0 symbols so far` over and over until the slow-timeout kills it.

Excerpt:
```
Initial LSP indexing: 9 symbols for /var/folders/.../src/lib.rs
Initial lsp_symbols count for lib.rs: 9
Poll re-index LSP: 0 symbols so far
Poll re-index LSP: 0 symbols so far
... (repeats until timeout)
```

This is a concrete gap against the project-memory note that file-watcher is still a stub ("File watcher is FileEvent enum only (no actual watching); Incremental invalidation not implemented"). The test expects incremental re-indexing on file change to work; the feature is not wired up yet.

What to do:
- If incremental re-indexing is planned, keep the test but mark progress toward implementing the file watcher + invalidation path.
- If the feature is not yet implemented, the test should not live in `workspace_e2e_test.rs` as an active test — it should be deleted or the feature implemented. Do NOT add `#[ignore]`.

File: `swissarmyhammer-code-context/tests/workspace_e2e_test.rs` (test: `test_lsp_reindexing_after_file_change`) #test-failure