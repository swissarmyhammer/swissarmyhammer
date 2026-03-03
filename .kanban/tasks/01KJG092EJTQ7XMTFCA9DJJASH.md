---
title: Migrate column/swimlane storage from board.json to individual files
position:
  column: done
  ordinal: a7
---
Refactor all column and swimlane operations to use file-based storage instead of embedding in board.json. This is the core architectural change for git-friendly merging.

**Board struct changes** (types/board.rs):
- Remove `columns: Vec<Column>` and `swimlanes: Vec<Swimlane>` from Board struct
- Board becomes just `{name, description}` - the slim, merge-friendly board.json
- Keep Column/Swimlane structs and Board helper methods, but they now read from context
- Board helper methods (find_column, first_column, terminal_column, find_swimlane) either move to context or become free functions that take a columns slice

**Column operations** (column/add.rs, get.rs, update.rs, delete.rs, list.rs):
- AddColumn: write individual column file via ctx.write_column() instead of modifying board.columns
- GetColumn: read via ctx.read_column() instead of board.find_column()
- UpdateColumn: read column file, modify, write back instead of modifying board
- DeleteColumn: check tasks, then ctx.delete_column_file() instead of filtering board.columns
- ListColumns: ctx.read_all_columns() instead of board.columns

**Swimlane operations** (swimlane/add.rs, get.rs, update.rs, delete.rs, list.rs):
- Same pattern as columns - all operations read/write individual files

**Board operations**:
- InitBoard (board/init.rs): write slim board.json + write 3 default column files (todo.json, doing.json, done.json) via ctx.write_column()
- GetBoard (board/get.rs): read columns via ctx.read_all_columns(), swimlanes via ctx.read_all_swimlanes(), assemble the response JSON the same way (response format unchanged)

**Migration** (context.rs or processor.rs):
- On board read, if board.json still contains columns/swimlanes arrays:
  1. Write each column/swimlane to individual files
  2. Rewrite board.json without columns/swimlanes
- This makes migration automatic and transparent

**Other operations that reference board.columns**:
- task/add.rs: validates column exists → use ctx.read_column() or ctx.read_all_columns()
- task/mv.rs: validates column/swimlane → same
- task/complete.rs: finds terminal column → ctx.read_all_columns(), find max order
- task/next.rs: finds first column → ctx.read_all_columns(), find min order
- board/get.rs: builds column counts → already addressed above

Files: All files in column/, swimlane/, board/, task/ dirs + types/board.rs + context.rs
Verify: cargo test -p swissarmyhammer-kanban — ALL existing tests must pass. The on-disk format changes but the API behavior is identical.