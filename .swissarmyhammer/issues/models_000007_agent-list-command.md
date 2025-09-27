# Step 7: Agent List Command Implementation

Refer to ideas/models.md

## Objective

Implement the `sah agent list` command with proper output formatting following existing patterns.

## Tasks

### 1. Implement Core List Functionality
- Complete `list.rs` implementation in `swissarmyhammer-cli/src/commands/agent/`
- Use `AgentManager::list_agents()` from config library
- Handle discovery errors gracefully with user-friendly messages
- Return appropriate exit codes on success/failure

### 2. Add Output Formatting Support
- Support table format (default) following prompt command patterns  
- Support JSON format with `--format json`
- Support YAML format with `--format yaml`
- Use existing `OutputFormat` enum from CLI

### 3. Implement Table Display
- Create `display_agents_table()` function
- Show summary line with counts: "🤖 Agents: X total"
- Show source breakdown: "📦 Built-in: X", "📁 Project: X", "👤 User: X"
- Use two-line format for each agent:
  - Line 1: "Name | Description" (colored by source)
  - Line 2: "  source: builtin" (dimmed)
- Add blank line between entries

### 4. Add Color Support
- Use `colored` crate for output formatting
- Different colors by source: built-in (green), project (yellow), user (blue)
- Respect TTY detection with `atty` crate
- Graceful fallback for non-TTY output

### 5. Add Error Handling
- Handle agent discovery errors with helpful messages
- Show "No agents found" for empty results
- Continue with partial results if some agents fail to load
- Log warnings for invalid agents

## Implementation Notes

- Follow the exact pattern from `prompt list` command
- Use same color scheme and formatting approach
- Keep table output clean and easy to scan
- Add appropriate tracing for debugging

## Acceptance Criteria

- `sah agent list` shows all agents with proper formatting
- Output formats (table, JSON, YAML) all work correctly
- Colors work in TTY and gracefully degrade in non-TTY
- Error messages are helpful and actionable
- Command follows existing CLI patterns exactly

## Files to Modify

- `swissarmyhammer-cli/src/commands/agent/list.rs`
- `swissarmyhammer-cli/src/commands/agent/mod.rs` (routing)