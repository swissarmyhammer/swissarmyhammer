---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff880
title: 'Coverage: get_implementations tree-sitter fallback in ops/get_implementations.rs'
---
crates/code-context/src/ops/get_implementations.rs

Coverage: 23% (7/30 lines)

The tree-sitter fallback path that searches for impl blocks when no LSP is available. Index a file containing a trait and its impl blocks, then verify the fallback returns correct implementation locations. Cover: trait with impls, trait with no impls, struct with impl blocks. #coverage-gap