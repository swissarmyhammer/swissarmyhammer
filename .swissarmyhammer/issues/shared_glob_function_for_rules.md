# Add shared file globbing function for rule checking

## Problem
Both CLI and MCP rule checking need to glob files (default: `**/*.*`) while respecting `.gitignore`. This logic should be shared to avoid duplication.

## Goal
Create a shared file globbing utility in `swissarmyhammer-rules` that both CLI and MCP can use.

## Requirements
- Create a public function that globs files with configurable pattern
- Default pattern: `**/*.*`
- Must respect `.gitignore` (use existing glob implementation from swissarmyhammer-core if available)
- Return list of file paths
- Should handle errors gracefully (invalid patterns, permission issues, etc.)

## Implementation Notes
- Check if there's already a glob function in swissarmyhammer-core that respects .gitignore
- If so, expose it through swissarmyhammer-rules or use it directly
- If not, implement it in the appropriate location
- Ensure it's consistent with other file operations in the codebase

## Dependencies
- Should be done after or alongside `consolidate_rule_check_logic`