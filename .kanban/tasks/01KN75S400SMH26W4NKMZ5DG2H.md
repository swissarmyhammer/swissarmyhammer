---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8780
title: 'Test file tools: edit, glob, grep, read, write edge cases'
---
Multiple files in swissarmyhammer-tools/src/mcp/tools/files/:

- edit/mod.rs (68.8%, 44 uncovered) - error paths for invalid edits, multi-match handling
- glob/mod.rs (76.5%, 36 uncovered) - glob pattern edge cases, directory filtering
- grep/mod.rs (75.8%, 23 uncovered) - regex error handling, context line options
- shared_utils.rs (72.2%, 70 uncovered) - path validation, content truncation, encoding edge cases
- write/mod.rs (67.7%, 21 uncovered) - write permission errors, directory creation
- mod.rs (68.1%, 15 uncovered) - file tool dispatch/registration
- read/mod.rs (77.8%, 8 uncovered) - binary file detection, offset/limit edge cases

Total: ~217 uncovered lines across the file tools subsystem. #coverage-gap