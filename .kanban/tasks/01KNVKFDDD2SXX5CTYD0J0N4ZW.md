---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: Add tests for LspDaemon::start failure paths
---
daemon.rs:156-267\n\nCoverage: 74.6% (153/205 lines)\n\nUncovered lines: 160-161, 183-186, 191, 223-225, 230, 247-248, 254-255, 258\n\nFunctions:\n- `start()` — binary not found path (lines 159-168): `which::which` fails, state → NotFound\n- `start()` — spawn failure path (lines 183-186): Command::spawn returns Err\n- `start()` — stderr filter task (lines 223-225, 230): spawns tokio task to filter stderr lines\n- `start()` — pipe conversion failures (lines 247-248, 254-258): into_owned_fd fails, or stdin/stdout unavailable\n\nTest scenarios:\n- Mock binary not on PATH → verify BinaryNotFound error, state = NotFound\n- Spawn failure (bad command) → verify SpawnFailed error\n- Pipe conversion failure edge case\n- stderr filtering with config\n\n#coverage-gap #coverage-gap