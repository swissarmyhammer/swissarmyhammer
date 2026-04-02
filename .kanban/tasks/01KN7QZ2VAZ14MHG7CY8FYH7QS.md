---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffa480
title: Add tests for process_texts with progress reporting and report_progress
---
model-embedding/src/batch.rs:297-324,483-520\n\nCoverage gap in process_texts non-empty path and report_progress helper.\n\nNeed tests exercising progress reporting with callback that verifies ProgressInfo fields, and the estimated_remaining_ms computation.