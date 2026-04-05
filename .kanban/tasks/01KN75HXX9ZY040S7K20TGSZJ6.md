---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9680
title: 'Test LSP worker loop: client unavailability, batch processing, error recovery'
---
File: swissarmyhammer-code-context/src/lsp_worker.rs (42.1%, 73 uncovered lines)\n\nUncovered functions:\n- `spawn_lsp_indexing_worker()` (lines 82-107): thread spawn and error path\n- `run_lsp_indexing_loop()` (lines 114-212): shutdown signaling, empty extensions warning, dirty file query, client lock poisoned recovery, client unavailable sleep, per-file indexing with error marking\n- `index_single_file()` (lines 221-252): file read, didOpen, collect_and_persist, didClose, error warning\n\nThese are integration-heavy (threading, mutex, LSP). Consider:\n- Unit test with mock SharedLspClient (None) to test unavailable path\n- Test shutdown flag terminates loop\n- Test index_single_file with mock client\n\nAcceptance: coverage >= 60% for lsp_worker.rs #coverage-gap