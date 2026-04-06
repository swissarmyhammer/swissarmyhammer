---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff280
title: 'Fix: Live LSP ops need atomic lock hold for didOpen+request+didClose sequence'
---
## What

Live LSP ops (get_hover, get_definition, get_references, etc.) send a 3-step sequence:
1. `lsp_notify(didOpen)`
2. `lsp_request(textDocument/hover)` 
3. `lsp_notify(didClose)`

Each of these independently acquires and releases the `SharedLspClient` mutex. Between steps, the indexing worker thread can interleave its own requests on the same stdin/stdout pipe, causing:
- Our hover response to be consumed by the worker
- The worker's response to be returned to us as the hover result
- Corrupted/mismatched responses

This is why `get_hover` and `get_definition` return `LspIndex` instead of `LiveLsp` — the live request fails silently due to interleaving.

## Root cause

`SharedLspClient` is `Arc<Mutex<Option<LspJsonRpcClient>>>`. `LspJsonRpcClient` owns the stdin/stdout pipes. Multiple callers (MCP handlers + indexing worker) share the same client.

`LayeredContext::lsp_notify()` and `lsp_request()` each lock/unlock independently. The 3-step sequence is NOT atomic.

## Fix

Add a method to `LayeredContext` that holds the lock for the entire didOpen+request+didClose sequence:

```rust
/// Execute a live LSP request with didOpen/didClose wrapping.
/// Holds the mutex for the entire sequence to prevent interleaving.
pub fn lsp_request_with_document(
    &self,
    file_path: &str,
    method: &str,
    params: Value,
) -> Result<Option<Value>, CodeContextError> {
    let client = match self.lsp_client {
        Some(c) => c,
        None => return Ok(None),
    };
    let mut guard = client.lock().map_err(...)?;
    let rpc = match guard.as_mut() {
        Some(rpc) => rpc,
        None => return Ok(None),
    };
    
    // All three ops under one lock hold:
    let uri = file_path_to_uri(file_path);
    let text = std::fs::read_to_string(file_path).unwrap_or_default();
    let lang = language_id_from_path(file_path);
    
    rpc.send_notification("textDocument/didOpen", json!({...}))?;
    let response = rpc.send_request(method, params)?;
    rpc.send_notification("textDocument/didClose", json!({...}))?;
    
    Ok(Some(response))
}
```

Then update all live LSP ops to use this instead of separate notify+request+notify calls.

## Files to modify
- `swissarmyhammer-code-context/src/layered_context.rs` — add `lsp_request_with_document()`
- `swissarmyhammer-code-context/src/ops/get_hover.rs` — use new method
- `swissarmyhammer-code-context/src/ops/get_definition.rs` — use new method
- `swissarmyhammer-code-context/src/ops/get_type_definition.rs` — use new method
- `swissarmyhammer-code-context/src/ops/get_references.rs` — use new method
- `swissarmyhammer-code-context/src/ops/get_implementations.rs` — use new method
- `swissarmyhammer-code-context/src/ops/get_inbound_calls.rs` — may need variant for multi-request sequences
- `swissarmyhammer-code-context/src/ops/get_diagnostics.rs` — uses pull diagnostics, same issue
- `swissarmyhammer-code-context/src/ops/get_rename_edits.rs` — uses multi-request sequence
- `swissarmyhammer-code-context/src/ops/get_code_actions.rs` — uses multi-request sequence

## Acceptance Criteria
- [ ] `lsp_request_with_document()` holds mutex for entire didOpen+request+didClose
- [ ] All live LSP ops use the new atomic method
- [ ] `get hover` returns `source_layer: \"LiveLsp\"` when rust-analyzer is running
- [ ] No interleaving between MCP handlers and indexing worker
- [ ] All tests pass

#lsp-live