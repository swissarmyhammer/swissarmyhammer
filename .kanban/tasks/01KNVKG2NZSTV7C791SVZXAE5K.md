---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
title: Add tests for async send/read_jsonrpc_message error paths
---
daemon.rs:540, 554, 556-558, 561-562, 566, 579, 585, 589-590, 596-598, 608-611\n\nCoverage: 74.6% (153/205 lines)\n\nUncovered lines: 540, 554, 556-558, 561-562, 566, 579, 585, 589-590, 596-598, 608-611\n\nFunctions:\n1. `graceful_shutdown()` (line 540) — wait() returns Err → ShutdownFailed\n2. `send_jsonrpc_message()` (lines 554-566) — write header error, write body error, flush error\n3. `read_jsonrpc_message()` (lines 579-614) — read_line error, EOF, bad Content-Length parse, missing Content-Length, read_exact error, json decode error\n\nThese are pub async functions that can be tested directly with tokio::io::Cursor or similar mock readers/writers.\n\nTest scenarios:\n- send_jsonrpc_message with broken writer → verify each error variant\n- read_jsonrpc_message with EOF → \"unexpected EOF\"\n- read_jsonrpc_message with missing Content-Length → error\n- read_jsonrpc_message with bad Content-Length → parse error\n- read_jsonrpc_message with truncated body → read error\n- read_jsonrpc_message with invalid JSON body → decode error\n\n#coverage-gap #coverage-gap