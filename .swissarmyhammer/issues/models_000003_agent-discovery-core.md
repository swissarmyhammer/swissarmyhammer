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