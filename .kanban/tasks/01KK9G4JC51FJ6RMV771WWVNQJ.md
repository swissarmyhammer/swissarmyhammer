---
position_column: done
position_ordinal: ffffffa480
title: 'command-scope.test.tsx: invoke("log_command") returns undefined in test - 4 failures'
---
All 4 failures in src/lib/command-scope.test.tsx share the same root cause: `invoke("log_command", { cmd, target })` at command-scope.tsx:183 returns undefined in the test environment, so `.catch()` fails with TypeError. Tests affected: (1) executes a resolved command and returns true, (2) executes parent command when child does not register it, (3) handles async execute functions, (4) dispatchCommand > calls execute when set. Fix: mock `invoke` to return a Promise for the `log_command` call. #test-failure