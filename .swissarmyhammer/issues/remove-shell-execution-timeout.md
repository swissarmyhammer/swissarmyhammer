# Remove Shell Command Execution Timeout (Redundant with MCP Timeout)

## Problem

The shell command execution system has its own timeout configuration in `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`, but shell commands are always called via MCP which already has its own timeout mechanism. This creates redundant timeout layers.

## Current Shell Execution Timeouts

Located in `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`:
- `DEFAULT_MIN_TIMEOUT`: 1 second
- `DEFAULT_MAX_TIMEOUT`: 1800 seconds (30 minutes)  
- `DEFAULT_DEFAULT_TIMEOUT`: 3600 seconds (1 hour)
- Shell execute tool accepts `timeout` parameter (5-1800 seconds)

## MCP Server Timeout Already Exists

- **MCP Server Timeout**: 900 seconds (15 minutes) default
- **Location**: `swissarmyhammer-config/src/agent.rs:150`
- **Purpose**: Controls all MCP communication including shell commands

## Rationale for Removal

### MCP Timeout Provides Sufficient Protection
- All shell commands go through MCP server
- MCP server timeout (15 minutes default) prevents hanging
- No shell command should run longer than MCP allows anyway
- Double timeout layers create confusion

### Simplifies Shell Tool API
- Remove `timeout` parameter from shell execute requests
- Remove timeout validation logic from shell tool
- Simpler parameter structure for shell commands
- Less configuration for users to understand

### Reduces Timeout Hierarchy Complexity
- Eliminates potential conflicts between shell timeout and MCP timeout
- Single point of timeout control at MCP level
- More predictable behavior - commands timeout when MCP times out

## Implementation Tasks

### 1. Remove Timeout Constants
- Remove `DEFAULT_MIN_TIMEOUT`, `DEFAULT_MAX_TIMEOUT`, `DEFAULT_DEFAULT_TIMEOUT`
- Remove timeout validation logic in shell execute tool
- Remove `timeout` field from shell execute request parameters

### 2. Update Shell Execute Tool
- Remove timeout parameter from tool description
- Remove timeout handling from command execution
- Simplify shell command execution to rely on MCP timeout
- Update tool schema and parameter validation

### 3. Update Documentation and Examples
- Remove timeout examples from shell tool documentation
- Update any CLI examples that specify shell timeouts
- Remove timeout references from shell tool description
- Update integration examples that use shell timeouts

### 4. Update Tests
- Remove shell timeout validation tests
- Update integration tests that specify shell timeouts
- Ensure shell commands work properly with MCP timeout only
- Test that shell commands properly respect MCP timeout

## Benefits After Removal

- Simplified shell command API
- Single timeout control point (MCP level)
- Reduced configuration complexity
- Elimination of timeout conflict scenarios
- Cleaner separation of concerns (MCP handles timing, shell handles execution)

## Files to Update

- `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` - Main implementation
- `swissarmyhammer-tools/src/mcp/tools/shell/execute/description.md` - Tool description
- Shell tool tests and integration tests
- Documentation mentioning shell command timeouts
- CLI help text for shell commands
- Any examples using shell timeout parameters