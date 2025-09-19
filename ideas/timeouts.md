# Timeout Configuration Analysis

Based on analysis of the SwissArmyHammer codebase, here are all the distinct timeout configurations that can be set:

## Core Configurable Timeouts

### 1. MCP Server Timeout (`timeout_seconds`)
- **Location**: `swissarmyhammer-config/src/agent.rs:150`
- **Default**: 900 seconds (15 minutes) 
- **Purpose**: MCP server communication timeouts

### 2. Shell Security Check Timeout (`security_check_timeout`)
- **Location**: `swissarmyhammer-shell/src/hardening.rs:52`
- **Default**: 5 seconds
- **Purpose**: Shell command security validation

### 3. Shell Execution Timeouts
- **Location**: `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`
- **Constants**:
  - `DEFAULT_MIN_TIMEOUT`: 1 second
  - `DEFAULT_MAX_TIMEOUT`: 1800 seconds (30 minutes)
  - `DEFAULT_DEFAULT_TIMEOUT`: 3600 seconds (1 hour)
- **Purpose**: Command execution timeout limits

### 4. Web Fetch Timeouts
- **Location**: `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs`
- **Default**: 30 seconds
- **Range**: 5-120 seconds
- **Purpose**: HTTP request timeouts

### 5. Workflow Action Timeouts (3 types)
- **Location**: `swissarmyhammer-workflow/src/actions.rs:111-117`
- **Types**:
  - `prompt_timeout`: For prompt actions
  - `user_input_timeout`: For user input actions  
  - `sub_workflow_timeout`: For sub-workflow actions
- **Purpose**: Individual workflow action timeouts

### 6. Web Search Content Fetch Timeout
- **Location**: `swissarmyhammer-tools/src/mcp/tools/web_search/content_fetcher.rs:81`
- **Default**: 45 seconds
- **Purpose**: Web content fetching during searches



## CLI/Workflow Timeouts (configurable)

### 8. CLI Command Timeouts
- **Location**: `swissarmyhammer-cli/src/cli.rs:262,279,371`
- **Commands**: Flow run/resume/test commands accept `timeout` parameter
- **Purpose**: Overall CLI command execution timeouts

### 9. Overall Workflow Timeout
- **Configuration**: `timeout_ms` in workflow YAML configurations
- **Range**: Found in documentation examples ranging from 30 seconds to 1 week
- **Purpose**: Complete workflow execution timeout

## Summary

Total: **9 distinct timeout categories** that can be configured, with some categories having multiple related timeout types (like the 3 workflow action timeout types).

These timeouts cover the full execution pipeline from shell commands to web requests to complete workflow execution.