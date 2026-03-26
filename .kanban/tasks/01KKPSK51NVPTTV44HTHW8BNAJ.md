---
position_column: done
position_ordinal: ffffffffd480
title: Discovery file errors reuse LockFileCreation variant
---
**discovery.rs:53,56,75**\n\n`write_discovery` and `read_discovery` map IO errors to `ElectionError::LockFileCreation`, which is semantically wrong — these are discovery file operations, not lock file creation failures. Callers matching on error variants will be misled.\n\n**Suggestion**: Add a `Discovery(#[source] io::Error)` variant to `ElectionError` and use it in discovery.rs.\n\n**Verify**: `cargo test -p swissarmyhammer-leader-election` passes after change.