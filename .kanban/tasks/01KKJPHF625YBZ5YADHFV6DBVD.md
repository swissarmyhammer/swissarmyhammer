---
position_column: done
position_ordinal: b680
title: 'FAIL: swissarmyhammer-tools integration::file_size_limits::test_shell_execute_handles_large_output'
---
Test panicked at swissarmyhammer-tools/tests/integration/file_size_limits.rs:442:5\nassertion failed: response_text.contains(\"Line 1\")\n\nThe shell_execute large output test asserts that truncated output still contains \"Line 1\" from the beginning, but it appears the output is not being returned as expected. #test-failure