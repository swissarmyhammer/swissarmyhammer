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
## Proposed Solution

I will implement the `AgentManager::list_agents()` function with the following approach:

### 1. Agent Precedence Logic
- Start with built-in agents as the base list
- Override with project agents (same name replaces built-in entry at same position)  
- Override with user agents (same name replaces any existing entry at same position)
- New agents from project/user sources are appended after existing agents

### 2. Implementation Steps
1. **Write failing test** - Test the complete precedence system with mock agents
2. **Implement list_agents()** - Core function with proper precedence handling
3. **Add error handling** - Comprehensive error handling with context
4. **Add validation** - Validate agent YAML during loading process
5. **Add tracing** - Debug logging for agent discovery and precedence

### 3. Error Handling Strategy
- Continue processing if individual agent files fail validation
- Log warnings for invalid agents but don't fail entire operation
- Provide specific context for which agent file failed
- Handle I/O errors gracefully (missing directories, permission issues)

### 4. Key Design Decisions  
- Use "find and replace" logic to maintain original ordering
- Built-in agents provide the base ordering structure
- Project/user agents either override existing positions or append new entries
- Maintain comprehensive tracing for debugging complex precedence scenarios


## Implementation Complete ✅

### What Was Implemented

#### 1. Main `AgentManager::list_agents()` Function
- ✅ Combines all agent sources with proper precedence hierarchy
- ✅ Built-in agents (lowest precedence) → Project agents (medium) → User agents (highest)
- ✅ Agent overriding by name works correctly - same position replacement
- ✅ New agents from higher precedence sources are appended to the list

#### 2. Agent Precedence Logic
- ✅ Find-and-replace logic maintains original ordering from built-in agents
- ✅ Project agents override built-in agents by name at the same position
- ✅ User agents override any existing agent (built-in or project) by name
- ✅ Comprehensive tracing for debugging precedence resolution

#### 3. Agent Validation and Error Handling  
- ✅ Individual agent files are validated using `AgentConfig` deserialization
- ✅ Invalid agents are skipped with warning logs but don't break entire loading process
- ✅ Missing directories handled gracefully (return empty vector)
- ✅ I/O errors logged with context but processing continues for other agents
- ✅ Comprehensive error context for failed agent files

#### 4. Comprehensive Testing
- ✅ All 192 tests passing
- ✅ Tests for basic precedence functionality  
- ✅ Tests for agent overriding scenarios with temporary directories
- ✅ Tests for validation error handling (invalid YAML, invalid configs)
- ✅ Tests for edge cases (empty directories, non-YAML files)
- ✅ Tests for I/O error scenarios

### Key Implementation Details

**Agent Loading Flow:**
1. Load built-in agents using `load_builtin_agents()` 
2. Load project agents from `./agents/` directory
3. Load user agents from `~/.swissarmyhammer/agents/`
4. Apply precedence: user > project > builtin

**Error Handling Strategy:**
- Individual agent validation failures are logged but don't prevent loading others
- Missing directories are handled gracefully (no error)
- I/O errors for individual files are logged but processing continues
- Only critical errors (like failure to load built-in agents) cause the function to fail

**Tracing and Debugging:**
- Debug logs show which agents are loaded from each source
- Trace logs show individual agent loading and precedence decisions
- Info logs provide summary of final agent discovery results
- Comprehensive logging for troubleshooting complex precedence scenarios

### Files Modified
- `swissarmyhammer-config/src/agent.rs` - Added `list_agents()` function and improved error handling

### Test Coverage
- ✅ 54 agent-related tests all passing  
- ✅ Full test suite (192 tests) passing
- ✅ Comprehensive coverage of precedence scenarios
- ✅ Error handling and validation edge cases covered
- ✅ Integration tests with temporary directories and file system operations