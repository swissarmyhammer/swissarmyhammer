---
position_column: done
position_ordinal: t2
title: 'CODE-CONTEXT-FIX-1: Implement SQLite database schema initialization'
---
Create the SQLite schema that is completely missing.

**What needs to happen:**
1. Implement schema creation in swissarmyhammer-code-context crate (or appropriate location)
2. Create 4 tables per spec lines 24-106:
   - indexed_files: PRIMARY KEY file_path, content_hash (BLOB), file_size, last_seen_at, ts_indexed INTEGER, lsp_indexed INTEGER
   - ts_chunks: id INTEGER PRIMARY KEY, file_path TEXT, start_byte/end_byte, start_line/end_line, text TEXT, symbol_path TEXT, embedding BLOB
   - lsp_symbols: id TEXT PRIMARY KEY, name, kind INTEGER, file_path, start/end line/char, detail TEXT
   - lsp_call_edges: id INTEGER PRIMARY KEY, caller_id, callee_id, from_ranges TEXT (JSON), source TEXT ('lsp' or 'treesitter')
3. Add foreign key constraints with ON DELETE CASCADE
4. Add indexes per spec

**Why this is critical:** Without this, tree-sitter parsing results cannot be persisted. ALL query operations return empty.

**Quality Test Criteria:**
- cargo build succeeds
- Unit test: create database, verify all 4 tables exist with correct columns
- Unit test: insert test data, verify foreign keys work
- Unit test: ON DELETE CASCADE works (delete indexed_files row removes associated chunks/symbols/edges)