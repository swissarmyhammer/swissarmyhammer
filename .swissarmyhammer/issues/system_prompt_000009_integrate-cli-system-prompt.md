# Integrate System Prompt with CLI Claude Code Invocation

Refer to /Users/wballard/github/swissarmyhammer/ideas/system_prompt.md

## Overview
Integrate the system prompt rendering with all Claude Code CLI invocations, using the `--append-system-prompt` parameter to inject the rendered `.system.md` content.

## Prerequisites
- Issue system_prompt_000008 (system prompt rendering) completed
- System prompt rendering infrastructure implemented and tested
- CLI integration research completed

## Implementation Scope

### 1. CLI Integration Points
Based on investigation, identify and modify all locations where Claude Code is invoked:
- **Prompt Commands**: `sah prompt render`, `sah prompt test`, etc.
- **Workflow Commands**: `sah workflow run`, etc.
- **Direct CLI Invocations**: Any direct claude code calls
- **MCP Integration**: Any claude code calls through MCP

### 2. Parameter Injection Logic
- **System Prompt Rendering**: Call system prompt renderer before Claude Code invocation
- **Parameter Construction**: Build `--append-system-prompt "rendered_content"` parameter
- **Error Handling**: Handle system prompt rendering failures gracefully
- **Fallback Strategy**: Define behavior when system prompt rendering fails

### 3. Integration Architecture
- **Centralized Function**: Create single function for Claude Code invocation with system prompt
- **Parameter Management**: Handle all Claude Code parameters consistently
- **Error Propagation**: Maintain existing error handling patterns
- **Logging**: Add appropriate logging for debugging

## Technical Implementation

### Function Design
```rust
pub fn invoke_claude_code_with_system_prompt(
    args: &[String],
    additional_params: Option<Vec<String>>
) -> Result<Output, CliError> {
    // 1. Render system prompt
    // 2. Add --append-system-prompt parameter
    // 3. Invoke Claude Code
    // 4. Return result with proper error handling
}
```

### Integration Strategy
1. **Replace Direct Calls**: Replace all direct `claude code` calls with new function
2. **Maintain Compatibility**: Ensure all existing functionality continues working
3. **Add System Prompt**: Inject system prompt seamlessly
4. **Error Handling**: Handle system prompt failures without breaking existing flows

## Implementation Steps

1. **Create centralized Claude Code invocation function**
   - Implement function with system prompt integration
   - Add comprehensive error handling
   - Include logging and debugging support

2. **Update all CLI command handlers**
   - Replace direct claude code calls
   - Test each command type individually
   - Ensure parameter passing works correctly

3. **Update workflow integration**
   - Modify workflow execution to use new function
   - Test workflow compatibility
   - Verify no regression in workflow functionality

4. **Add configuration options**
   - Option to enable/disable system prompt injection
   - Debug mode for troubleshooting system prompt issues
   - Configuration validation and error handling

## Success Criteria
- ✅ All Claude Code invocations use system prompt integration
- ✅ System prompt content properly injected via `--append-system-prompt`
- ✅ No regression in existing CLI functionality
- ✅ Comprehensive error handling for system prompt failures
- ✅ Proper logging and debugging capabilities
- ✅ Configuration options working correctly

## Testing Requirements
- **Integration Tests**: Test all CLI commands with system prompt
- **Error Handling Tests**: Test behavior when system prompt rendering fails
- **Compatibility Tests**: Ensure no regression in existing functionality
- **Configuration Tests**: Test enable/disable system prompt options
- **Performance Tests**: Verify no significant performance impact

## Risk Mitigation
- **Gradual Rollout**: Update commands incrementally
- **Fallback Strategy**: Continue working even if system prompt fails
- **Thorough Testing**: Comprehensive test coverage before deployment
- **Rollback Plan**: Easy rollback if issues are discovered

## Technical Notes
- Maintain backward compatibility with existing CLI patterns
- Ensure system prompt injection is transparent to users
- Consider performance impact of system prompt rendering
- Plan for future enhancements and customization options