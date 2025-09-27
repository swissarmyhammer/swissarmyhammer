# Step 5: Agent Use/Configuration Application Functionality

Refer to ideas/models.md

## Objective

Implement the `use_agent()` functionality to apply selected agent configurations to project config files.

## Tasks

### 1. Add Agent Search Function
- Add `find_agent_by_name()` helper function
- Search through all discovered agents by name
- Return `Result<AgentInfo, AgentError::NotFound>`
- Use existing `list_agents()` for discovery

### 2. Add Config File Detection
- Add function to detect existing project config file
- Check for `.swissarmyhammer/sah.yaml` first
- Check for `.swissarmyhammer/sah.toml` as fallback
- Return path to existing file or default to `.swissarmyhammer/sah.yaml`

### 3. Implement Config File Creation
- Add function to ensure `.swissarmyhammer/` directory exists
- Create new config file if none exists
- Use YAML format for new files (`.swissarmyhammer/sah.yaml`)
- Include minimal structure with agent section

### 4. Implement Agent Configuration Application
- Add `AgentManager::use_agent(agent_name: &str)` function
- Parse agent YAML content to `AgentConfig`
- Load existing project config or create new one
- Replace/update the `agent:` section in project config
- Write updated config back to disk

### 5. Add Configuration Validation
- Validate agent configuration before applying
- Ensure required agent fields are present
- Provide clear error messages for invalid configurations
- Validate config file write permissions

## Implementation Notes

- Use `serde_yaml` for YAML manipulation
- Preserve other sections in config files when updating agent section
- Create backup of existing config before modification
- Follow existing config loading patterns in the codebase

## Acceptance Criteria

- `use_agent()` successfully applies agent configurations
- Existing config files are updated correctly without loss of other settings
- New config files are created with proper structure when needed
- Error handling covers all failure scenarios (permissions, invalid YAML, etc.)
- Configuration validation prevents invalid states

## Files to Modify

- `swissarmyhammer-config/src/agent.rs`