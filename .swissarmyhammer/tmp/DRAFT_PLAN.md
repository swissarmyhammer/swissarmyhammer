# Draft Plan: MCP Abort Tool Implementation

## Specification Analysis
Based on the `specification/abort.md` file, we need to implement a comprehensive replacement for the brittle string-based "ABORT ERROR" detection system with a robust MCP tool that allows controlled termination.

## Current Implementation Analysis
The existing system uses string-based detection for "ABORT ERROR" patterns across:
- CLI error detection (multiple locations)
- Workflow system execution
- Test files for abort testing
- Built-in prompts

## Problem to Solve
Replace the unreliable string-based abort detection with a file-based MCP tool approach that is:
- More robust across process boundaries
- Language/framework agnostic
- Easier to test and maintain
- Provides atomic operations

## Implementation Strategy

### Phase 1: Foundation Setup
1. **Project Setup and MCP Tool Infrastructure**
   - Set up the abort tool in the MCP tool registry
   - Create the basic abort tool structure following the noun/verb pattern
   - Implement tool registration and parameter validation

2. **Core Abort Tool Implementation**
   - Implement the `abort` MCP tool with required parameters
   - Add file creation logic to `.swissarmyhammer/.abort`
   - Ensure atomic file operations and proper error handling

### Phase 2: Workflow System Integration
3. **WorkflowRun Cleanup Integration**
   - Add abort file cleanup logic to `WorkflowRun::new()`
   - Ensure proper error handling for cleanup operations
   - Add appropriate logging for cleanup operations

4. **Executor Integration**
   - Add file-based abort detection to `execute_state_with_limit`
   - Implement the abort check loop before each workflow iteration
   - Add proper error handling and logging

5. **Error Type Extension**
   - Add new `ExecutorError::Abort` variant
   - Update error propagation throughout the system
   - Ensure proper error context preservation

### Phase 3: CLI Integration and Error Handling
6. **CLI Error Handling Updates**
   - Update CLI error handling to detect `ExecutorError::Abort`
   - Remove string-based abort detection from CLI code
   - Maintain proper exit codes and error messaging

7. **Built-in Prompt Updates**
   - Update `builtin/prompts/abort.md` to use the new MCP tool
   - Ensure the prompt provides clear instructions for the tool usage
   - Update any other prompts that reference abort functionality

### Phase 4: Testing and Migration
8. **Comprehensive Testing**
   - Unit tests for abort tool functionality
   - Integration tests for end-to-end abort flow
   - Tests for file cleanup and error propagation
   - Regression tests to ensure existing behavior is maintained

9. **String-Based System Removal**
   - Remove all string-based "ABORT ERROR" detection code
   - Update test files that relied on string-based detection
   - Clean up obsolete modules and functions

### Phase 5: Documentation and Cleanup
10. **Documentation Updates**
    - Update workflow documentation to reflect new abort system
    - Add MCP tool documentation for the abort tool
    - Update error handling patterns in memos

11. **Final Integration Testing**
    - End-to-end testing across all components
    - Performance validation
    - Cross-platform compatibility testing

## Implementation Details

### File-Based Approach
- Use `.swissarmyhammer/.abort` for abort state storage
- Plain text file containing abort reason
- Atomic file creation for thread safety

### MCP Tool Specification
```json
{
  "tool": "abort", 
  "parameters": {
    "reason": "User cancelled the destructive operation"
  }
}
```

### Error Flow
1. MCP tool creates abort file with reason
2. Workflow executor detects file in main loop
3. ExecutorError::Abort is raised with reason
4. CLI handles error and exits with appropriate code

## Key Benefits
- Reliability through file-based detection
- Testability through simple file operations
- Maintainability with single source of truth
- Cross-process compatibility
- Atomic operations

## Migration Strategy
The plan follows a phased approach:
1. Add new system alongside existing
2. Update usage to new system
3. Remove old string-based system
4. Comprehensive testing and documentation

This approach ensures backward compatibility during migration and provides comprehensive testing at each phase.