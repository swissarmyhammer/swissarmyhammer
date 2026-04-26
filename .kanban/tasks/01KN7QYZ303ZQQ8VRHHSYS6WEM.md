---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffac80
title: Add tests for process_file and process_file_streaming
---
model-embedding/src/batch.rs:327-412\n\nCoverage: 55.4% (128/231 lines)\n\nUncovered: process_file() and process_file_streaming() — file-based batch processing. Need tempfile-based tests for: valid file, empty file, file not found, file with blank lines, streaming callback.