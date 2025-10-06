# Implement MCP rule_check tool

## Problem
Need to expose rule checking functionality through MCP for AI agents and external tools.

## Goal
Create an MCP tool `rule_check` that allows checking rules against files.

## Requirements
- Tool name: `rule_check`
- Parameters:
  - `rule_names` (optional): Array of rule names to run. If not provided, run all rules
  - `file_paths` (optional): Array of file paths to check. If not provided, glob `**/*.*` using shared glob function
- Use consolidated rule checking logic from swissarmyhammer-rules crate
- Return structured results with:
  - Rule violations (with severity, file, line, message)
  - Warnings
  - Errors
  - Summary statistics

## Implementation Notes
- Add tool definition to MCP server in `swissarmyhammer-mcp`
- Follow existing MCP tool patterns in the codebase
- Use the shared API from `consolidate_rule_check_logic` issue
- Use the shared glob function from `shared_glob_function_for_rules` issue
- Format output appropriately for MCP consumers

## Dependencies
- `consolidate_rule_check_logic` must be completed first
- `shared_glob_function_for_rules` must be completed first