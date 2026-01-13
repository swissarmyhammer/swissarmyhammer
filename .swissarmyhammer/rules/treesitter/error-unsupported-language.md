---
severity: error
tags:
- error-handling
- validation
---

# Tree-sitter: Unsupported Language Error

## Acceptance Criterion
**AC-23**: Unsupported language returns structured error with supported extensions list

## What to Check
When given file with unsupported extension, tool must:
- Return error with code `TS_UNSUPPORTED_LANGUAGE`
- Include file path and extension in error details
- Provide list of supported extensions
- Not crash or return generic error

## Success Criteria
- Error response matches specification format exactly
- Error code is `TS_UNSUPPORTED_LANGUAGE`
- Details include: file_path, extension, supported_extensions
- Supported extensions list is complete and accurate

## Reference
See specification/treesitter.md - Unsupported Language error section