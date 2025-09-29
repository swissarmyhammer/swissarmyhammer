change the `sah agent list` display visuals to work like the `sah prompt list` logic using a display type and tabled

## Proposed Solution

After analyzing the `sah prompt list` implementation, I need to refactor the `sah agent list` command to follow the same pattern:

### Current Implementation Issues
- Uses custom `display_agents_table()` function with manual formatting
- Manually handles different output formats (JSON/YAML) instead of using CliContext
- Has complex custom table display logic with colors and summaries
- Doesn't leverage the standard `Tabled` derive pattern

### Changes Required
1. **Create display.rs module** for agent command similar to `prompt/display.rs`
   - `AgentRow` struct with `#[derive(Tabled, Serialize, Deserialize)]` for standard output
   - `VerboseAgentRow` struct for detailed output with verbose flag
   - `DisplayRows` enum to handle both row types
   - `agents_to_display_rows()` conversion function

2. **Update list.rs** to use the standard pattern:
   - Remove custom `display_agents_table()` function
   - Remove manual JSON/YAML serialization
   - Use `cli_context.display()` method like prompt list does
   - Simplify the execute_list_command function

3. **Maintain functionality**:
   - Keep the source-based agent grouping (builtin/project/user)
   - Preserve agent descriptions and source information
   - Support verbose mode for additional details

This will make agent list consistent with prompt list and leverage the framework's standard display handling.

## Implementation Complete

Successfully refactored `sah agent list` to follow the same display pattern as `sah prompt list`:

### Changes Made

1. **Created `display.rs` module** (`swissarmyhammer-cli/src/commands/agent/display.rs`):
   - `AgentRow` struct with `Tabled`, `Serialize`, `Deserialize` derives for standard output
   - `VerboseAgentRow` struct with additional "Content Size" field for verbose mode
   - `DisplayRows` enum to handle both row types
   - `agents_to_display_rows()` conversion function

2. **Updated `list.rs`** to use standard framework pattern:
   - Removed custom `display_agents_table()` function
   - Removed manual JSON/YAML serialization 
   - Now uses `context.display_agents()` method like prompt list does
   - Simplified execute_list_command function

3. **Added `display_agents()` method** to `CliContext` in `context.rs`:
   - Handles both standard and verbose display rows consistently
   - Leverages existing `display()` method for actual output formatting

### Testing Results
- All tests pass including new display module tests
- Standard mode shows: Name | Description | Source (as tabled output)
- Verbose mode shows: Name | Description | Source | Content Size (as tabled output) 
- JSON/YAML output formats work through framework's standard serialization

### Functionality Preserved
- Agent discovery and loading still works correctly
- Source-based grouping (builtin/project/user) information maintained
- Error handling and logging preserved
- All existing command-line options continue to work

The implementation now fully matches the `prompt list` pattern while maintaining all existing functionality.