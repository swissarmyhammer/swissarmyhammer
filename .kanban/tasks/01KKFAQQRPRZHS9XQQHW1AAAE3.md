---
position_column: done
position_ordinal: ffae80
title: '[warning] lsp_worker infinite loop has no shutdown mechanism'
---
**Severity: warning**\n**File:** swissarmyhammer-code-context/src/lsp_worker.rs:119\n\nThe `run_lsp_indexing_loop` function contains a `loop { ... }` with no break condition and no shutdown signal (e.g., `Arc<AtomicBool>` or channel-based cancellation). The function signature returns `Result<(), CodeContextError>`, but in practice it can never return `Ok(())` -- only panic or error. The only way to stop this thread is process termination. Consider adding a cancellation token or `AtomicBool` flag for graceful shutdown."