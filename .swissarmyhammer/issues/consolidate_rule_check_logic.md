# Consolidate rule checking logic into swissarmyhammer-rules crate

## Problem
Rule checking logic currently exists in the CLI. We need to add MCP support for rule checking, and we must avoid duplicating logic between CLI and MCP.

## Goal
Create a shared, reusable rule checking API in the `swissarmyhammer-rules` crate that both CLI and MCP can use.

## Requirements
- Extract core rule checking logic from CLI into swissarmyhammer-rules
- Create a public API that accepts:
  - Optional list of rule names (None = run all rules)
  - List of file paths to check
- Return structured results (violations, warnings, errors)
- Handle rule loading, file processing, and result aggregation
- Should NOT handle output formatting (that's CLI/MCP specific)

## Implementation Notes
- Look at current CLI rule check implementation in `swissarmyhammer-cli`
- Identify what logic belongs in the rules crate vs. CLI/MCP presentation layer
- Consider creating a `RuleChecker` struct or similar abstraction
- Ensure the API is ergonomic for both CLI and MCP consumers

## Dependencies
None - this should be done first