---
assignees:
- assistant
position_column: done
position_ordinal: ffffff9b80
title: 'Fix flaky test: test_lsp_reindexing_after_file_change'
---
## What
`swissarmyhammer-code-context::workspace_e2e_test::test_lsp_reindexing_after_file_change` fails intermittently in the full workspace test run. This is a race condition in the LSP reindexing test — not related to the XDG/.sah changes.

## Acceptance Criteria
- [ ] Test passes reliably under `cargo nextest run` parallel execution
- [ ] No `#[ignore]` or `#[serial]` band-aid — fix the underlying race condition
- [ ] Run 10x in a loop to confirm stability

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-code-context -E 'test(test_lsp_reindexing)'` passes 10/10