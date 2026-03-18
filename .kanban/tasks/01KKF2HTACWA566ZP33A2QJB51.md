---
assignees:
- assistant
position_column: done
position_ordinal: ff8480
title: LSP send_request needs read timeout
---
In `lsp_communication.rs`, `send_request()` loops reading responses with no timeout. If the LSP server ignores or delays a response (e.g., rust-analyzer receiving a documentSymbol for a .toml file), the worker hangs forever on that file and never processes the rest.

Fix: Add a read timeout to the BufReader/stdin so `read_jsonrpc_response` returns an error after N seconds instead of blocking indefinitely.

Files: `swissarmyhammer-code-context/src/lsp_communication.rs` (send_request ~line 192)