---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffff480
title: 'Test LSP communication: call hierarchy, shutdown, JSON-RPC framing'
---
File: swissarmyhammer-code-context/src/lsp_communication.rs (60.3%, 126 uncovered lines)\n\nUncovered functions:\n- `collect_and_persist_file_symbols()` (lines 351-388): symbol collection with DB persistence and error paths\n- `initialize()` (lines 391-418): LSP initialize request with error response handling\n- `shutdown()` (lines 520-537): shutdown + exit notification\n- `collect_call_edges()` (lines 543-626): call hierarchy prepare, outgoing calls, edge collection\n- `collect_and_persist_call_edges()` (lines 629-639): edge persistence\n- `read_jsonrpc_response()` (lines 751-789): Content-Length header parsing, EOF, body read\n- `parse_call_hierarchy_items()` (lines 644-660): parse error/null/array\n- `parse_outgoing_calls()` (lines 663-679): parse error/null/array\n\nTests needed:\n- Unit tests for read_jsonrpc_response with valid/invalid/EOF input\n- Unit tests for parse_call_hierarchy_items, parse_outgoing_calls\n- Integration test for initialize/shutdown with mock stdio\n\nAcceptance: coverage >= 70% for lsp_communication.rs #coverage-gap