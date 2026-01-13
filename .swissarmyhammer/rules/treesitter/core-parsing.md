---
severity: error
tags:
- treesitter
- parsing
- performance
---

# Core Parsing Requirements

From specification/treesitter.md - AC1: Core Parsing

The treesitter implementation MUST:

- Handle syntax errors gracefully with partial results
- Support all 25+ languages listed in specification
- Auto-detect language from file extension
- Return parse errors without crashing

## Verification

- Syntax errors are reported but do not cause crashes
- All 25+ languages from the specification are supported
- Language detection works correctly from file extensions
- Parse errors are returned in structured format
