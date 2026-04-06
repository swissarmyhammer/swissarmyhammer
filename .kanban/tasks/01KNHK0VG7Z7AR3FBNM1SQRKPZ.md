---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff780
title: 'Coverage: try_treesitter in ops/get_inbound_calls.rs'
---
crates/code-context/src/ops/get_inbound_calls.rs

Coverage: 10% (3/29 lines)

Tree-sitter fallback path for finding inbound callers when no LSP is available. Set up a code-context database with indexed files and call edges, then invoke try_treesitter and verify it returns correct caller information. Cover: callers found, no callers, depth > 1 recursion. #coverage-gap