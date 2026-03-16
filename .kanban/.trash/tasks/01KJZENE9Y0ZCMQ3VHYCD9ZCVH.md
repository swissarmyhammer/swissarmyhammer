---
position_column: done
position_ordinal: k9
title: Fix test_validate_file_path_relative
---
File: swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs:971. Panic: unwrap on Err - Os { code: 2, kind: NotFound, message: "No such file or directory" }. Test depends on filesystem state that does not exist. #test-failure