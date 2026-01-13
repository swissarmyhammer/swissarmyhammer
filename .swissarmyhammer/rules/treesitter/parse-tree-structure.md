---
severity: error
tags:
- operations
- parse
---

# Tree-sitter: Parse Operation Tree Structure

## Acceptance Criterion
**AC-14**: `parse` operation returns syntax tree structure with all top-level symbols

## What to Check
The parse operation must return:
- Language detected from file extension
- Root node type of syntax tree
- List of all top-level symbols (functions, classes, structs, etc.)
- Symbol details: name, kind, line, column

## Success Criteria
- Response includes `language` field
- Response includes `root_node` field  
- `symbols` array contains all top-level definitions
- Each symbol has name, kind, line, column

## Reference
See specification/treesitter.md - Parse Response format example