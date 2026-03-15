---
position_column: done
position_ordinal: ffe880
title: Fix test_validate_file_path_relative in swissarmyhammer-tools (No such file or directory)
---
Test at swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs:1008 panics with 'No such file or directory'. The test tries to canonicalize a relative path that doesn't exist in the test environment. #test-failure