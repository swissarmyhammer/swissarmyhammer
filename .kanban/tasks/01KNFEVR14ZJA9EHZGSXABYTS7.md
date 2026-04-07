---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffed80
title: lsp_notify sends a request+reads response instead of a true notification
---
swissarmyhammer-code-context/src/layered_context.rs, lsp_notify method\n\nThe `lsp_notify` method has a comment acknowledging the problem: 'Use send_request for now -- LSP notifications don't expect a response but our client always reads one. For true notifications we'd need a separate send path.'\n\nThis means every didOpen/didClose 'notification' actually blocks waiting for a response from the LSP server. Since LSP notifications are fire-and-forget by spec, many servers may not send a response at all, causing a 30-second timeout (LSP_REQUEST_TIMEOUT) on every call that uses didOpen/didClose.\n\nEvery new live LSP op calls didOpen + action + didClose, so this is 2 extra blocking round-trips per operation that should be fire-and-forget. For operations like get_references that call didOpen/didClose, this doubles the latency.\n\nSuggestion: Add a `send_notification` method to LspJsonRpcClient that writes the JSON-RPC message without reading a response. The method signature already exists in the JSON-RPC spec: notifications have no 'id' field and expect no response." #review-finding