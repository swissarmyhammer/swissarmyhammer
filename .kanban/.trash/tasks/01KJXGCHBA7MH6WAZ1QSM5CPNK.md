---
position_column: done
position_ordinal: c3
title: 'Fix file path validator tests: 4 tests fail with "No such file or directory"'
---
4 tests in shared_utils fail because std::env::current_dir() returns NotFound. Tests: test_file_path_validator_relative_paths (line 1145), test_secure_file_access_read_relative_paths (line 1401), test_file_path_validator_relative_with_workspace (line 1191), test_validate_file_path_relative (line 971). File: /Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs