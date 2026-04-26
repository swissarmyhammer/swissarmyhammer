---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffdb80
title: Duplicated test helper functions across all op test modules
---
swissarmyhammer-code-context/src/ops/*.rs (all 10 new op files + layered_context.rs)\n\nEvery test module defines its own copies of `test_db()`, `insert_file()`, `insert_lsp_symbol()`, `insert_ts_chunk()`, and `insert_call_edge()`. These are 11 copies of nearly identical helper functions, with minor signature differences (e.g., some `insert_lsp_symbol` take a `detail` parameter, others don't).\n\nThis makes it easy for test helpers to diverge from the actual schema, which has already happened: some helpers insert with different column sets.\n\nSuggestion: Create a `#[cfg(test)] mod test_fixtures` in the crate root or a `test_helpers.rs` file that provides canonical versions of these helpers. Import them in each test module." #review-finding