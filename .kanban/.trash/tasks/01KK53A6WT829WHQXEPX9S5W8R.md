---
assignees:
- assistant
position_column: done
position_ordinal: s1
title: 'code_context tool: grep code'
---
## What
Implement `grep code` operation — regex search across stored chunk text, returning complete semantic blocks. Uses `regex` crate + `rayon::par_iter` for parallel matching.

Files: `swissarmyhammer-code-context/src/ops/grep_code.rs`

Spec: `ideas/code-context-architecture.md` — "grep code" section.

## Acceptance Criteria
- [ ] Compiles pattern once with `regex::Regex`
- [ ] Loads chunks from `ts_chunks`, `rayon::par_iter` across chunks to test matches
- [ ] Returns matching chunks with: file path, line range, symbol path, highlighted match positions, full chunk text
- [ ] Supports `language` filter, `files` filter, `max_results` cap
- [ ] Blocks until tree-sitter indexing is complete (with progress notification)

## Tests
- [ ] Unit test: regex `fn\s+\w+` matches function definitions, returns full function body
- [ ] Unit test: `max_results` caps output
- [ ] Unit test: `language` filter excludes non-matching chunks
- [ ] Unit test: no matches returns empty result, not error
- [ ] `cargo test -p swissarmyhammer-code-context`