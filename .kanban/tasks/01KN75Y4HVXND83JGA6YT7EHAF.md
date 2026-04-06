---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff380
title: Test LSP daemon start/shutdown/restart lifecycle
---
File: swissarmyhammer-lsp/src/daemon.rs (17.3% coverage, 36/208 lines)\n\nUncovered functions:\n- start() - binary check, spawn, initialize handshake, stderr drain, JSON-RPC client setup (lines 151-268)\n- shutdown() - graceful LSP shutdown sequence (lines ~270-320)\n- restart() - stop + start with backoff (lines ~328-345)\n- health_check() - check if child process is alive (lines ~351-378)\n- initialize_handshake() - LSP initialize/initialized protocol (lines ~381-434)\n- client() - mutex-guarded client access with poison recovery (lines 120-133)\n\nThis is the core LSP process management. Testing requires either a real LSP server binary or a mock stdin/stdout process. Consider testing with a simple echo server or the error paths (binary not found, spawn failure, timeout)." #coverage-gap