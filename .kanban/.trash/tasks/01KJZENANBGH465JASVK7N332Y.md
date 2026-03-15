---
position_column: done
position_ordinal: k7
title: Fix test_file_path_validator_relative_paths
---
File: swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs:1173. Assertion failure: "Should accept nested relative path". The file path validator incorrectly rejects nested relative paths. #test-failure