---
position_column: done
position_ordinal: c5
title: Fix test_secure_file_access_read_relative_paths - panics with 'Should be able to read file with ./ relative path'
---
Test in swissarmyhammer-tools/src/mcp/tools/files/shared_utils.rs:1414 panics: "Should be able to read file with ./ relative path". The secure file access logic fails to handle relative paths starting with "./".