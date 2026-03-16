---
position_column: done
position_ordinal: k1
title: 'Fix 4 file_path_validator relative path tests: CWD not found'
---
Tests in swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs panic with unwrap on Err "No such file or directory". Affected tests: test_file_path_validator_relative_paths (line 1145), test_secure_file_access_read_relative_paths (line 1401), test_file_path_validator_relative_with_workspace (line 1191), test_validate_file_path_relative (line 971). Same CWD race condition as the reload_prompts tests. #test-failure