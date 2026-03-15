---
position_column: done
position_ordinal: r9
title: Incremental invalidation (1-hop ReextractFile + RefreshEdges)
---
## What
When a file changes, re-extract its symbols and edges (generation 0), then refresh edges for files whose callees changed (generation 1). No further propagation.

Files: `swissarmyhammer-code-context/src/invalidation.rs`

Spec: `ideas/code-context-architecture.md` — "LSP layer: incremental invalidation" section.

## Acceptance Criteria
- [ ] `ReextractFile` — deletes old symbols + outgoing edges, re-queries LSP, writes fresh data
- [ ] `RefreshEdges` — keeps existing symbols, re-queries outgoing calls only (~10x cheaper)
- [ ] 1-hop propagation: diffs old vs new symbol IDs, finds files with reverse edges to deleted symbols, enqueues `RefreshEdges` for each
- [ ] `RefreshEdges` never triggers further propagation (closes the loop)
- [ ] Watcher integration: `LspWatcherHandler` triggers `ReextractFile` for changed files

## Tests
- [ ] Unit test: file F has symbol A calling G.foo. Delete A from F, verify reverse lookup finds G, G gets RefreshEdges
- [ ] Unit test: RefreshEdges does NOT trigger further propagation
- [ ] Unit test: symbol rename in F triggers 1-hop to callers of old symbol name
- [ ] `cargo test -p swissarmyhammer-code-context`