# Step 4: Agent List Implementation with Discovery Hierarchy

Refer to ideas/models.md

## Objective

Complete the agent discovery system by implementing the main `list_agents()` function with full hierarchy support.

## Tasks

### 1. Implement Main List Function
- Add `AgentManager::list_agents()` function
- Combine all agent sources with proper precedence
- Return `Result<Vec<AgentInfo>, AgentError>`
- Handle agent overriding by name (user > project > builtin)

### 2. Implement Agent Precedence Logic
- Start with built-in agents as base
- Override with project agents (same name replaces)
- Override with user agents (same name replaces)
- Maintain original order within each source

### 3. Add Agent Validation
- Validate agent YAML content during loading
- Use existing `AgentConfig` deserialization for validation
- Include validation errors in `AgentError` types
- Continue loading other agents if one fails validation

### 4. Add Comprehensive Error Handling
- Handle I/O errors from directory scanning
- Handle YAML parsing errors from agent files
- Provide context for which agent file failed
- Log warnings for invalid agents but continue processing

### 5. Add Unit Tests
- Test built-in agent loading
- Test directory-based loading with various scenarios
- Test agent precedence and overriding
- Test error cases (missing dirs, invalid YAML, etc.)

## Implementation Notes

- Use `replace` logic: find existing agent by name and replace entire entry
- Preserve order: built-in first, then project additions, then user additions
- For overrides, use the position of the original agent
- Add comprehensive tracing for debugging

## Acceptance Criteria

- `list_agents()` returns all agents with correct precedence
- Agent overriding works correctly by name matching
- Invalid agents don't break the entire listing
- Comprehensive test coverage for all scenarios
- Error messages are helpful and specific

## Files to Modify

- `swissarmyhammer-config/src/agent.rs`