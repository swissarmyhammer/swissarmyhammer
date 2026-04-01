---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffcc80
title: Add tests for PerspectiveChangelog::append and read_all error paths
---
File: swissarmyhammer-perspectives/src/changelog.rs:120-160\n\nCoverage: 86.1% (31/36 lines in changelog.rs)\n\nUncovered lines: 130 (empty-line skip in read_all), 140 (non-NotFound error in read_all), 147, 154, 158 (append body)\n\nThe append method (private, called by log_create/log_update/log_delete) is not directly tested in isolation. Tarpaulin marks the actual file-write lines as uncovered even though the log_* tests call append -- this may be an async instrumentation gap. The read_all method has two uncovered branches: skipping blank lines and propagating non-NotFound IO errors.\n\nWhat to test:\n1. Write a changelog file with blank/whitespace-only lines interspersed and verify read_all skips them.\n2. Verify that a non-NotFound IO error (e.g., read a directory path instead of a file) propagates as an error from read_all.\n3. Add a round-trip test that explicitly asserts the JSONL file content on disk after append (verifying the file format). #coverage-gap