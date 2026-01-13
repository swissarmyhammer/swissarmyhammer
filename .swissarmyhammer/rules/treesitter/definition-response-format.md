---
severity: error
tags:
- operations
- definition
- response-format
---

# Tree-sitter: Definition Response Format

## Acceptance Criterion
**AC-7**: Returns file path, line, column, kind (function/class/method), and preview for each definition

## What to Check
Each definition in the response must include:
- `file_path`: Absolute path to file containing definition
- `line`: Line number (1-based)
- `column`: Column number (1-based)  
- `kind`: Type of definition (function, class, method, struct, etc.)
- `preview`: Source code preview of the definition line

## Success Criteria
- Response structure matches specification exactly
- All 5 fields present for each definition
- Line/column numbers are 1-based (not 0-based)
- Preview shows actual source code

## Reference
See specification/treesitter.md - Definition Response format example