---
assignees:
- claude-code
depends_on:
- 01M53GC28P974P34HEP7XN651M
position_column: todo
position_ordinal: fffe80
title: Do not send ignored paths to the LSP
---
## Problem
The LSP worker (`crates/swissarmyhammer-code-context/src/lsp_worker.rs`) reads rows with `lsp_indexed = 0`. Then it sends `didOpen`, `documentSymbol` and call-hierarchy requests for each file. It does not check the path. When sourcekit-lsp opens a file in `.build/checkouts/<pkg>/`, it finds `<pkg>/Package.swift`. Then it makes a new workspace and starts a full background index for that package. The diagnostics watcher and `LspSession::sync_open` (`session.rs`) also do not check the path.

## Fix
- Put a guard at the single point where sah opens a document in the LSP (`LspSession::sync_open` or the call that it uses). If the shared ignore filter skips the path, or the path is outside the workspace root, do not send the request. Return a clear result (for example, skipped) and do not return an error.
- In the LSP worker, mark skipped rows so that the worker does not read them again in a loop.

## Acceptance criteria
- For an ignored path or a path outside the root, sah sends no `didOpen` and no other LSP request.
- The worker does not read the same skipped rows again and again.

## Tests
- A unit test with a fake LSP client that records the requests. Give it one ignored path and one normal path. Only the normal path gets requests. #lsp-fix #bug