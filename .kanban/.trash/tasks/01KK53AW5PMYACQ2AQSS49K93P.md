---
assignees:
- assistant
position_column: done
position_ordinal: s4
title: 'code_context tool: find symbol + search symbol + list symbol'
---
## What
Three symbol query operations: `find symbol` (exact location lookup), `search symbol` (fuzzy/prefix workspace search), `list symbol` (all symbols in a file).

Files: `swissarmyhammer-code-context/src/ops/find_symbol.rs`, `src/ops/search_symbol.rs`, `src/ops/list_symbol.rs`

Spec: `ideas/code-context-architecture.md` — "find symbol", "search symbol", "list symbol" sections.

## Acceptance Criteria
- [ ] `find symbol`: returns definition location (file, line, char) for a symbol by name
- [ ] `search symbol`: fuzzy/prefix search across all symbols, filterable by `kind` (function, method, struct, class, interface)
- [ ] `list symbol`: all symbols in a specific file, enriched with LSP type detail when available
- [ ] All three draw from both tree-sitter and LSP symbol tables
- [ ] Block until relevant layer indexed

## Tests
- [ ] Unit test: `find symbol "MyStruct"` returns file path + line
- [ ] Unit test: `search symbol` with query `"auth"` returns matching symbols
- [ ] Unit test: `search symbol` with `kind: "function"` filters out structs
- [ ] Unit test: `list symbol` for a file returns all symbols sorted by line
- [ ] `cargo test -p swissarmyhammer-code-context`