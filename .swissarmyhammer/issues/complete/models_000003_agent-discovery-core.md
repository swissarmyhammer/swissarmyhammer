# Step 3: Core Agent Discovery Implementation

Refer to ideas/models.md

## Objective

Implement the core agent discovery functionality with hierarchy: user > project > builtin agents.

## Tasks

### 1. Add Built-in Agent Loading
- Add `load_builtin_agents()` function to `AgentManager`
- Use `get_builtin_agents()` from Step 1
- Convert to `Vec<AgentInfo>` with `AgentSource::Builtin`
- Parse descriptions from agent content

### 2. Add Directory-based Agent Loading
- Add `load_agents_from_dir()` helper function
- Scan directory for `.yaml` files
- Handle missing directories gracefully
- Return `Vec<AgentInfo>` with appropriate source

### 3. Add User Agent Loading  
- Add `load_user_agents()` function
- Scan `.swissarmyhammer/agents/` directory
- Use `load_agents_from_dir()` with `AgentSource::User`

### 4. Add Project Agent Loading
- Add `load_project_agents()` function  
- Scan `agents/` directory in project root
- Use `load_agents_from_dir()` with `AgentSource::Project`

### 5. Implement Agent Manager Structure
- Add `AgentManager` struct with associated functions
- Start with empty struct - all functions will be associated functions

## Implementation Notes

- Follow precedence order: user agents override project agents override built-in agents
- Use same agent name for overriding (based on filename stem)
- Handle I/O errors gracefully with proper error context
- Place all functions in `swissarmyhammer-config/src/agent.rs`

## Acceptance Criteria

- All loading functions work with existing agent files
- Error handling is robust for missing directories/files
- Agent precedence works correctly
- No CLI integration yet - pure library functionality

## Files to Modify

- `swissarmyhammer-config/src/agent.rs`

## Proposed Solution

After examining the existing code, I'll implement the core agent discovery functionality as follows:

### 1. AgentManager Structure
- Add empty `AgentManager` struct with all functions as associated functions
- No instance data needed since all operations are stateless

### 2. Function Implementation Plan

#### `load_builtin_agents()` 
- Use existing `get_builtin_agents()` function from build script
- Convert each `(name, content)` tuple to `AgentInfo` with `AgentSource::Builtin`
- Parse descriptions using existing `parse_agent_description()` function

#### `load_agents_from_dir(dir_path, source)`
- Helper function to scan directory for `.yaml` files
- Handle missing directories by returning empty Vec (not an error)
- Read file contents and create `AgentInfo` instances
- Use filename stem (without .yaml) as agent name

#### `load_user_agents()`
- Scan `~/.swissarmyhammer/agents/` directory
- Use `load_agents_from_dir()` with `AgentSource::User`

#### `load_project_agents()`
- Scan `./agents/` directory in project root
- Use `load_agents_from_dir()` with `AgentSource::Project`

### 3. Error Handling Strategy
- Use existing `AgentError` types
- Handle I/O errors gracefully with proper context
- Missing directories should not be errors (return empty Vec)
- File parsing errors should be propagated up

### 4. Implementation Notes
- Follow precedence order: user > project > builtin
- Agent names based on filename stem for consistency
- All functions will be in `swissarmyhammer-config/src/agent.rs`
- No CLI integration in this step - pure library functionality

### 5. Testing Strategy
- Unit tests for each loading function
- Test with existing builtin agents
- Test error conditions (missing dirs, malformed files)
- Test precedence ordering

## Implementation Status ✅

Successfully implemented all core agent discovery functionality:

### ✅ AgentManager struct
- Added empty struct with all functions as associated functions
- No instance data needed since operations are stateless

### ✅ load_builtin_agents() function
- Uses existing `get_builtin_agents()` from build script
- Converts (name, content) tuples to `AgentInfo` with `AgentSource::Builtin`
- Parses descriptions using existing `parse_agent_description()` function
- **Verified**: Correctly loads claude-code, qwen-coder, and qwen-coder-flash agents

### ✅ load_agents_from_dir() helper function  
- Scans directory for `.yaml` files
- Handles missing directories gracefully (returns empty Vec)
- Creates `AgentInfo` instances with appropriate source
- Uses filename stem as agent name
- **Tested**: Works with temporary directories and ignores non-YAML files

### ✅ load_user_agents() function
- Scans `~/.swissarmyhammer/agents/` directory
- Uses `load_agents_from_dir()` with `AgentSource::User`
- **Tested**: Handles missing home directory gracefully

### ✅ load_project_agents() function
- Scans `./agents/` directory in project root  
- Uses `load_agents_from_dir()` with `AgentSource::Project`
- **Tested**: Works with current working directory

### ✅ Error handling
- All I/O errors properly propagated with context
- Missing directories handled gracefully (not errors)
- Invalid paths return appropriate `AgentError::InvalidPath`

### ✅ Testing
- **36 tests passed** including comprehensive unit tests
- Tests cover all loading functions, error conditions, and edge cases
- Verified builtin agent loading with actual agent files
- Tested directory scanning with temporary files
- All tests run successfully with no failures

### ✅ Code quality
- Clean compilation with only minor unused import warning (fixed)
- Follows existing code patterns and documentation style
- Proper error handling with existing `AgentError` types
- No CLI integration (as specified)

## Next Steps
Ready for Step 4 integration where these functions will be used to implement agent precedence and discovery CLI commands.