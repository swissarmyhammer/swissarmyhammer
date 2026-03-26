---
position_column: done
position_ordinal: f880
title: 'LSP daemon: rust-analyzer handshake fails with EOF'
---
## Problem

The LSP daemon fails to connect to rust-analyzer with `JSON-RPC error: unexpected EOF reading headers`. It spawns the process successfully but the initialize handshake never completes. The daemon then retries with backoff and fails again.

## Log Output

```
swissarmyhammer_lsp::daemon: LSP server spawned cmd="rust-analyzer" pid=98127
swissarmyhammer_lsp::daemon: LSP initialize failed cmd="rust-analyzer" reason=JSON-RPC error: unexpected EOF reading headers
swissarmyhammer_lsp::daemon: State transition cmd="rust-analyzer" state=Failed { reason: "JSON-RPC error: unexpected EOF reading headers", attempts: 2 }
swissarmyhammer_lsp::daemon: Restarting LSP server after backoff cmd="rust-analyzer" attempt=2 delay_secs=2
```

## Likely Causes

- stdin/stdout pipe setup incorrect — rust-analyzer may be writing to stderr or closing stdout early
- Missing required initialization params (rootUri, capabilities) that cause rust-analyzer to exit immediately
- rust-analyzer not found or wrong version spawned
- The daemon may not be setting `Content-Length` headers correctly in the JSON-RPC protocol

## Key Files

- `swissarmyhammer-lsp/src/daemon.rs` — spawns process and does initialize handshake
- `swissarmyhammer-lsp/src/supervisor.rs` — manages daemon lifecycle
- `swissarmyhammer-lsp/src/registry.rs` — server specs (command, args)