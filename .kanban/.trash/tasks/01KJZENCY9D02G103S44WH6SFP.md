---
position_column: done
position_ordinal: k8
title: Fix test_file_path_validator_relative_with_workspace
---
File: swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs:1219. Panic: unwrap on Err - Os { code: 2, kind: NotFound, message: "No such file or directory" }. Test depends on filesystem state that does not exist. #test-failure