---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8280
title: 'Coverage: layered_context.rs — ts_symbols_in_file pure index test'
---
swissarmyhammer-code-context/src/layered_context.rs

Coverage: 56.8% (187/329) — most uncovered lines are live-LSP methods (skipped). This card covers `ts_symbols_in_file` which is testable as a pure index query.

`ts_symbols_in_file` (lines 533-566) queries ts_chunks WHERE symbol_path IS NOT NULL and maps results to SymbolInfo structs. It extracts the short name from the qualified path via rsplit("::").

Uncovered paths:
1. The happy path — querying chunks with non-null symbol_path and returning SymbolInfo vec.
2. The name extraction logic — rsplit("::").next() to get short name from qualified path like "module::Struct::method".
3. The error/empty path — when no chunks have symbol_path set, returns empty vec.
4. The prepare error path (line 541) — returns empty vec on SQL error.

What to test:
- Create an in-memory DB with the code-context schema (use crate::db::create_schema).
- Insert rows into indexed_files and ts_chunks with symbol_path values.
- Construct a LayeredContext with a read-only connection to the DB.
- Call ts_symbols_in_file and assert: correct number of symbols, correct name extraction from qualified paths, correct range mapping (start_line, end_line), correct file_path propagation.
- Test with no symbol_path rows (should return empty vec).
- Test with a qualified path containing "::" to verify rsplit extraction.
- Test with a simple name (no "::") to verify the unwrap_or fallback.

#coverage-gap #code-context