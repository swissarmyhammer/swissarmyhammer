---
position_column: done
position_ordinal: f1
title: 'Fix shell execute tests: INITIAL_CWD Lazy poisoning in parallel test runs'
---
32 shell execute tests in swissarmyhammer-tools fail when run alongside other workspace crates. Root cause: a static `Lazy<PathBuf>` called `INITIAL_CWD` at `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs:33` panics with "CWD must be accessible at startup" (No such file or directory) when another test changes the CWD before this Lazy is initialized. All subsequent tests then fail with "Lazy instance has previously been poisoned". Tests pass when run in isolation (`cargo test -p swissarmyhammer-tools`). Fix: make INITIAL_CWD initialization resilient (fallback to a temp dir or use a per-test setup) or avoid relying on process-global CWD.