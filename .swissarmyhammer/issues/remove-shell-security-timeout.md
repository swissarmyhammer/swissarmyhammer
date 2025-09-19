# Remove Shell Security Check Timeout (Redundant with MCP Timeout)

## Problem

The shell security check system has its own timeout configuration in `swissarmyhammer-shell/src/hardening.rs:52` with a `security_check_timeout` field (default: 5 seconds). However, shell security checks are performed as part of shell command execution, which is always called via MCP that already has its own timeout mechanism. This creates redundant timeout layers.

## Current Shell Security Timeout

Located in `swissarmyhammer-shell/src/hardening.rs`:
- `security_check_timeout: Duration` field 
- Default: 5 seconds (`Duration::from_secs(5)` at line 91)
- Used for shell command security validation timeout

## MCP Server Timeout Already Exists

- **MCP Server Timeout**: 900 seconds (15 minutes) default
- **Location**: `swissarmyhammer-config/src/agent.rs:150`  
- **Purpose**: Controls all MCP communication including shell commands and their security checks

## Rationale for Removal

### MCP Timeout Provides Sufficient Protection
- All shell commands (including security checks) go through MCP server
- MCP server timeout (15 minutes) is much longer than security check timeout (5 seconds)
- Security validation should be fast - if it takes >15 minutes, something is seriously wrong
- Double timeout layers create unnecessary complexity

### Security Checks Should Be Fast
- Security validation operations should complete quickly (well under 15 minutes)
- If security check is slow enough to need its own timeout, it's likely broken
- MCP timeout is more than adequate to catch truly hanging security checks
- Security failures should fail fast, not timeout

### Simplifies Security Architecture
- Removes one timeout configuration from security system
- Single point of timeout control at MCP level
- Cleaner separation of concerns (MCP handles timing, security handles validation)
- Reduces configuration complexity in security hardening

## Implementation Tasks

### 1. Remove Security Check Timeout Field
- Remove `security_check_timeout: Duration` from hardening configuration struct
- Remove timeout initialization in default configuration
- Remove any timeout parameter passing to security check functions

### 2. Update Security Check Implementation
- Remove timeout handling from security validation logic
- Remove any `tokio::time::timeout` wrappers around security operations
- Simplify security check execution to rely on MCP timeout only
- Update security check function signatures if they accept timeout parameters

### 3. Update Security Configuration
- Remove timeout configuration from security hardening setup
- Update any configuration parsing that handles security timeouts
- Simplify security configuration structure
- Update default configuration values

### 4. Update Tests and Documentation
- Remove security timeout tests
- Update security documentation to remove timeout references
- Remove timeout examples from security configuration
- Ensure security tests work properly with MCP timeout only

## Benefits After Removal

- Simplified security configuration
- Single timeout control point (MCP level)
- Reduced configuration complexity
- Elimination of redundant timeout mechanisms
- Cleaner security architecture
- Less potential for timeout conflicts

## Files to Update

- `swissarmyhammer-shell/src/hardening.rs` - Security hardening configuration
- Security check implementation functions
- Security configuration parsing and setup
- Security tests and validation
- Documentation mentioning security timeouts
- Any examples using security timeout configuration