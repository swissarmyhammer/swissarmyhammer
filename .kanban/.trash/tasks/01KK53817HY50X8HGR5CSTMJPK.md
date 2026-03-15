---
position_column: done
position_ordinal: o3
title: 'swissarmyhammer-code-context: crate skeleton + unified DB schema'
---
## What
Create the `swissarmyhammer-code-context` crate with the unified SQLite schema (`indexed_files`, `ts_chunks`, `lsp_symbols`, `lsp_call_edges`), `.code-context/` workspace layout, and `CodeContextWorkspace` shell.

Files: `swissarmyhammer-code-context/Cargo.toml`, `src/lib.rs`, `src/db.rs`, `src/workspace.rs`

Spec: `ideas/code-context-architecture.md` — "Workspace layout" + "Database schema" sections.

## Acceptance Criteria
- [ ] Crate compiles with `cargo check -p swissarmyhammer-code-context`
- [ ] `create_schema(conn)` creates all 4 tables with correct foreign keys and ON DELETE CASCADE
- [ ] `source` column inline in `lsp_call_edges` CREATE TABLE (not ALTER TABLE)
- [ ] `lsp_symbols.id` uses qualified path format: `"lsp:{file_path}:{qualified_path}"`
- [ ] `.code-context/` directory auto-created with auto-generated `.gitignore` containing `*`
- [ ] `index.db` opened in WAL mode
- [ ] Leader election via `swissarmyhammer-leader-election` pointed at `.code-context/leader.lock`
- [ ] `CodeContextWorkspace::open()` shell — leader/reader split, returns mode

## Tests
- [ ] Unit test: schema creation on in-memory SQLite, verify all tables exist
- [ ] Unit test: CASCADE delete — insert file + chunks + symbols + edges, delete file, verify all gone
- [ ] Unit test: `.code-context/.gitignore` auto-created with `*`
- [ ] `cargo test -p swissarmyhammer-code-context`