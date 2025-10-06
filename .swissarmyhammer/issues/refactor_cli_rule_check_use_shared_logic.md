# Refactor CLI rule check to use shared logic

## Problem
After consolidating rule checking logic into swissarmyhammer-rules, the CLI needs to be updated to use the shared implementation instead of its own logic.

## Goal
Update the CLI `rule check` command to use the consolidated rule checking API from swissarmyhammer-rules.

## Requirements
- Replace existing CLI rule checking implementation with calls to shared API
- Keep CLI-specific concerns (output formatting, terminal colors, etc.) in CLI
- Ensure all existing CLI behavior is preserved
- Verify all CLI tests still pass
- No duplication between CLI and MCP implementations

## Implementation Notes
- Update `swissarmyhammer-cli` rule check command
- Use the shared rule checking API from swissarmyhammer-rules
- Keep presentation logic (formatting, colors, user output) in CLI
- May need to adapt shared results to CLI output format
- Run tests to ensure no regressions

## Dependencies
- `consolidate_rule_check_logic` must be completed first
- Should be done in parallel with or after `implement_mcp_rule_check_tool`