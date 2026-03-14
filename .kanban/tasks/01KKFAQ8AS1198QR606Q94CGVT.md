---
position_column: done
position_ordinal: d1
title: '[warning] SQL format-string injection risk in query_lsp_dirty_files and mark_non_lsp_capable_files'
---
**Severity: warning**\n**Files:** swissarmyhammer-code-context/src/lsp_worker.rs:282, swissarmyhammer-code-context/src/cleanup.rs:199\n\n`query_lsp_dirty_files()` and `mark_non_lsp_capable_files()` build SQL via `format!` with extension strings interpolated directly into LIKE clauses. Currently the extensions come from hardcoded `&'static str` constants so there is no actual injection risk. However, this pattern is fragile -- if the extension source ever changes to user input or YAML config, it becomes exploitable. Consider using parameterized queries with `IN` clauses or building the SQL with `?` placeholders." #review-finding