---
assignees:
- assistant
position_column: done
position_ordinal: ff8d80
title: LSP worker indexes files no LSP server can handle
---
File discovery adds all PARSEABLE_EXTENSIONS (.toml, .yaml, .sh, etc.) to indexed_files with lsp_indexed=0. The LSP dirty-file query returns ALL lsp_indexed=0 files with no file-type filter. Files like .toml/.yaml/.sh have no LSP server, so they either hang (no timeout) or get sent to rust-analyzer which may not respond.

Fix: During startup_cleanup, mark files as lsp_indexed=1 if their extension has no matching LSP server. Or filter the dirty-file query to only include extensions the active LSP server supports.

Files:
- `swissarmyhammer-code-context/src/cleanup.rs` (startup_cleanup)
- `swissarmyhammer-code-context/src/lsp_worker.rs` (query_lsp_dirty_files)