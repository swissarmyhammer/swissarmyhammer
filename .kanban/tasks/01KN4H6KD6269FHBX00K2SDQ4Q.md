---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffca80
title: Add tests for PerspectiveContext::open initialization and persistence round-trip
---
File: swissarmyhammer-perspectives/src/context.rs:39-57\n\nCoverage: 72.6% (53/73 lines in context.rs)\n\nUncovered lines: 45, 46, 47, 50, 53, 54, 56\n\nTarpaulin marks the struct field initializers and debug! log in open() as uncovered. This is likely an async instrumentation artifact since the existing tests DO call open(). However, the open -> load_all -> reopen round-trip deserves a dedicated test that verifies:\n1. Opening a fresh directory creates it and returns an empty context.\n2. Opening a directory with pre-existing YAML files loads them all.\n3. The debug log fires with the correct perspective count (use tracing-test or tracing-subscriber to capture).\n\nThe existing persistence_survives_reopen test partially covers this, but a focused test confirming the directory-creation behavior and empty-state would improve confidence. #coverage-gap