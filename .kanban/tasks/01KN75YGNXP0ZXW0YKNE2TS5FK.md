---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff480
title: Test LSP supervisor spawn_all/shutdown_all/health_check orchestration
---
File: swissarmyhammer-lsp/src/supervisor.rs (28.8% coverage, 17/59 lines)\n\nUncovered functions:\n- spawn_all() - starts all configured LSP daemons (lines 50-65)\n- shutdown_all() - graceful shutdown of all daemons (lines 70-90)\n- health_check() - checks all daemons and restarts failed ones (lines 98-136)\n- restart_daemon() - restart a specific daemon by name (lines 149-150)\n\nThe supervisor orchestrates multiple LspDaemon instances. Tests can use mock daemon specs with nonexistent binaries to test error handling, or create integration tests with a real LSP server." #coverage-gap