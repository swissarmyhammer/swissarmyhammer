---
position_column: done
position_ordinal: s5
title: 'code_context tool: get callgraph + get blastradius'
---
## What
Implement `get callgraph` (traverse call edges with direction + depth) and `get blastradius` (reverse traversal impact summary). Both use LSP edges when available, fall back to tree-sitter heuristic edges.

Files: `swissarmyhammer-code-context/src/ops/get_callgraph.rs`, `src/ops/get_blastradius.rs`

Spec: `ideas/code-context-architecture.md` — "get callgraph" + "get blastradius" sections.

## Acceptance Criteria
- [ ] `get callgraph`: accepts symbol name or `file:line:char`, direction (`inbound`/`outbound`/`both`), depth 1–5
- [ ] Traverses `lsp_call_edges`, returns edges with source provenance (`lsp` or `treesitter`)
- [ ] Falls back to tree-sitter heuristic edges when LSP unavailable
- [ ] `get blastradius`: accepts file + optional symbol, max_hops 1–10
- [ ] Returns aggregated impact: count of affected files/symbols per hop, ranked by proximity
- [ ] Both block until relevant layer indexed

## Tests
- [ ] Unit test: A calls B calls C — `get callgraph A outbound depth:2` returns A→B→C
- [ ] Unit test: `get callgraph C inbound depth:1` returns B→C
- [ ] Unit test: `get blastradius` for C with max_hops:2 returns B (hop 1) and A (hop 2)
- [ ] Unit test: mixed provenance edges show correct `source` in results
- [ ] `cargo test -p swissarmyhammer-code-context`