---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: Add tests for daemon initialize_handshake and capture_stderr
---
daemon.rs:439, 449, 465, 487, 496\n\nCoverage: 74.6% (153/205 lines)\n\nUncovered lines: 439, 449, 465, 487, 496\n\nFunctions:\n1. `initialize_handshake()` (lines 426-489) — builds initialize request, sends it, reads response (with stderr capture on failure), sends initialized notification\n2. `capture_stderr()` (lines 493-503) — reads child stderr with timeout\n\nLines 439/449 are error paths (stdin/stdout unavailable, invalid workspace path). Line 465 is send_jsonrpc_message call. Line 487 is send initialized notification. Line 496 is no-stderr early return.\n\nTest scenarios:\n- Handshake with mock child that responds correctly → Ok\n- Handshake with child that closes stdout early → Err with stderr context\n- capture_stderr with no stderr pipe → empty string\n- Invalid workspace path → HandshakeFailed error\n\n#coverage-gap #coverage-gap