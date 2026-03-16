---
position_column: done
position_ordinal: s3
title: 'code_context tool: get symbol (fuzzy matching)'
---
## What
Implement `get symbol` — return full source text of a symbol by name with multi-tier fuzzy matching. The agent doesn't need to know which file it lives in.

Files: `swissarmyhammer-code-context/src/ops/get_symbol.rs`

Spec: `ideas/code-context-architecture.md` — "get symbol" section.

## Acceptance Criteria
- [ ] Multi-tier resolution: exact match → suffix match → case-insensitive → subsequence/fuzzy
- [ ] Suffix match: `authenticate` matches `MyStruct::authenticate`
- [ ] Fuzzy scored by edit distance, functions/methods ranked above modules
- [ ] Returns all candidates with scores, file paths, line ranges, full source text
- [ ] Draws from both `ts_chunks.symbol_path` and `lsp_symbols`, deduplicated by position
- [ ] Blocks until relevant layer is indexed

## Tests
- [ ] Unit test: exact match `MyStruct::new` returns one result
- [ ] Unit test: suffix match `new` returns multiple results ranked by specificity
- [ ] Unit test: case-insensitive `mystruct::NEW` still resolves
- [ ] Unit test: fuzzy `auth` matches `authenticate`, `AuthService`
- [ ] Unit test: no match returns empty, not error
- [ ] `cargo test -p swissarmyhammer-code-context`