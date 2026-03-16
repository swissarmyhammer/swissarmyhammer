---
assignees:
- assistant
position_column: done
position_ordinal: s0
title: Tree-sitter call graph heuristic
---
## What
When no LSP is available, generate approximate call edges by walking tree-sitter ASTs for `call_expression` nodes and matching callee names against known `symbol_path` values in `ts_chunks`. Edges tagged `source: 'treesitter'`.

Files: `swissarmyhammer-code-context/src/ts_callgraph.rs`

Spec: `ideas/code-context-architecture.md` — "Tree-sitter call graph heuristic" section.

## Acceptance Criteria
- [ ] For each chunk, parse AST and walk for `call_expression` (and language-equivalent nodes)
- [ ] Extract callee name: `foo()` → `"foo"`, `self.bar()` → `"bar"`, `MyStruct::new()` → `"MyStruct::new"`
- [ ] Look up callee name against all `symbol_path` values in `ts_chunks`
- [ ] On match, insert edge in `lsp_call_edges` with `source: 'treesitter'`
- [ ] Runs only when LSP is unavailable for the file's language
- [ ] Documented limitations: name collisions, dynamic dispatch, qualified paths, cross-language calls

## Tests
- [ ] Unit test: Rust file with `foo()` call, `foo` defined in another chunk → edge created
- [ ] Unit test: `self.method()` resolves to method in same struct's impl block
- [ ] Unit test: no match for unknown callee → no edge, no error
- [ ] Unit test: edges have `source: 'treesitter'`
- [ ] `cargo test -p swissarmyhammer-code-context`