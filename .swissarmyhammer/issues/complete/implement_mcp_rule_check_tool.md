# Implement MCP rule_check tool

## Problem
The rule checking functionality exists in the swissarmyhammer-rules crate but is not exposed through MCP for AI agents and external tools.

## Goal
Create an MCP tool `rule_check` that allows checking rules against files through the MCP interface.

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

## Architecture Constraint: Circular Dependency

Multiple circular dependencies exist in the crate structure:

```
swissarmyhammer-workflow → swissarmyhammer-tools
swissarmyhammer-rules → swissarmyhammer-workflow
swissarmyhammer-tools → swissarmyhammer-rules (attempted - creates cycle)

swissarmyhammer-tools defines McpTool trait
swissarmyhammer-rules → swissarmyhammer-tools (for McpTool - creates cycle)
```

This creates cycles that Cargo cannot resolve, regardless of where we place the implementation.

## Solution

Implement a CLI wrapper MCP tool in `swissarmyhammer-tools` that invokes the `sah rules check` CLI command.

### Rationale
- swissarmyhammer-tools cannot depend on swissarmyhammer-rules (circular dependency)
- swissarmyhammer-workflow cannot be used (rules → workflow → rules cycle)
- swissarmyhammer-rules cannot depend on tools (rules → workflow → tools cycle)
- CLI wrapper is the only viable approach that breaks all cycles
- Leverages existing, well-tested CLI functionality
- No code duplication required

### Tool Structure
```rust
pub struct RuleCheckTool {
    rule_checker: Arc<RuleChecker>,
}
```

### Parameters (JSON Schema)
- `rule_names` (optional): Array of strings - specific rule names to run
- `file_paths` (optional): Array of strings - file paths or glob patterns
  - If not provided, default to `["**/*.*"]`
  - Use `expand_glob_patterns` from `swissarmyhammer-common::glob_utils`

### Return Format
Structured MCP response with:
- Summary statistics (rules checked, files checked)
- List of violations with:
  - rule_name
  - file_path
  - severity (Error/Warning/Info/Hint)
  - message
- Errors if any occurred

## Implementation Plan

1. Create `swissarmyhammer-tools/src/mcp/tools/rules/check/` module structure
2. Implement `RuleCheckTool` with `McpTool` trait that:
   - Accepts `rule_names` and `file_paths` parameters
   - Invokes `sah rules check` CLI command via shell execution
   - Parses JSON output from CLI
   - Returns structured MCP response
3. Create `description.md` with comprehensive documentation
4. Register tool in MCP server initialization
5. Write comprehensive tests

## Dependencies
- `RuleChecker::check_with_filters()` exists in `swissarmyhammer-rules/src/checker.rs`
- `expand_glob_patterns()` exists in `swissarmyhammer-common/src/glob_utils.rs`
- MCP tool pattern established in `swissarmyhammer-tools/src/mcp/tools/`
- swissarmyhammer-rules crate does NOT depend on swissarmyhammer-tools (verified)
- swissarmyhammer-rules CAN depend on rmcp and schemars (need to add to Cargo.toml)
- McpTool trait is defined in swissarmyhammer-tools/src/mcp/tool_registry.rs

## Alternative Solutions (Not Chosen)

### Option 1: Implement in swissarmyhammer-rules
- Rules crate already has RuleChecker
- Would need to depend on swissarmyhammer-tools for McpTool trait
- Rejected: Creates circular dependency (rules → workflow → tools → workflow)

### Option 2: Implement in swissarmyhammer-workflow
- Workflow depends on both tools and rules
- Would have access to both McpTool and RuleChecker
- Rejected: Creates circular dependency (workflow → rules → workflow)

### Option 3: Direct dependency from tools to rules
- Most straightforward implementation
- Direct access to RuleChecker without CLI overhead
- Rejected: Creates circular dependency (tools → workflow → rules → workflow → tools)
