# Step 8: Agent Use Command Implementation

Refer to ideas/models.md

## Objective

Implement the `sah agent use <agent_name>` command to switch project agent configurations.

## Tasks

### 1. Implement Core Use Functionality
- Complete `use_command.rs` implementation in `swissarmyhammer-cli/src/commands/agent/`
- Use `AgentManager::use_agent()` from config library
- Handle agent not found errors with helpful suggestions
- Handle configuration update errors with clear messaging

### 2. Add Success Messaging
- Show confirmation message: "Successfully switched to agent: {agent_name}"
- Include agent source information (builtin/project/user)
- Show brief description if available
- Use consistent output formatting

### 3. Add Error Handling and Validation
- Validate agent name exists before attempting to use
- Handle permission errors for config file creation/modification
- Handle invalid agent configurations with descriptive errors
- Provide suggestions for similar agent names if exact match not found

### 4. Add Argument Validation
- Ensure agent_name argument is provided and non-empty
- Trim whitespace and validate format
- Provide helpful error messages for invalid inputs
- Support tab completion hints for future enhancement

### 5. Add Safety Features
- Show what will be changed before making modifications
- Create backup of existing config (if any) before changes
- Verify agent configuration is valid before applying
- Rollback on failure where possible

## Implementation Notes

- Use `AgentManager::use_agent()` for the actual config application
- Follow error handling patterns from other CLI commands
- Keep success messages concise but informative
- Add appropriate tracing for debugging config updates

## Acceptance Criteria

- `sah agent use qwen-coder` successfully switches agent configuration
- Error messages are clear and actionable for all failure cases
- Success messages provide confirmation and context
- Config file updates work correctly (new files and existing files)
- Command handles edge cases gracefully (permissions, invalid config, etc.)

## Files to Modify

- `swissarmyhammer-cli/src/commands/agent/use_command.rs`
- `swissarmyhammer-cli/src/commands/agent/mod.rs` (routing)