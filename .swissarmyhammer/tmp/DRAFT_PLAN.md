# SwissArmyHammer Implementation Plan

## Current State Analysis

After reviewing the codebase, I can see that:
- Most core specifications have been implemented and moved to `specification/complete/`
- There is one remaining specification: `specification/issue_current.md`
- All previous issues have been completed (244 total)
- Recent commits show completion of configuration and logging improvements

## Remaining Specification to Implement

### Issue Current Tool Refactor (`specification/issue_current.md`)
- **Purpose**: Remove `issue_current` and `issue_next` tools, consolidate into enhanced `issue_show` tool
- **Key Changes**: 
  - Enhance `issue_show` to accept "current" and "next" as special name parameters
  - Update 4 builtin prompt files to use new syntax
  - Remove old tool implementations
- **Complexity**: Medium - touches existing tools and builtin prompts

## Implementation Plan Structure

This will be broken into small, incremental steps following the established patterns in the codebase:

1. **Research Phase**: Understand current tool implementations and usage
2. **Enhancement Phase**: Extend `issue_show` tool with new functionality  
3. **Migration Phase**: Update builtin prompts to use new syntax
4. **Cleanup Phase**: Remove deprecated tools and update registry
5. **Testing Phase**: Comprehensive testing of new functionality
6. **Documentation Phase**: Update tool descriptions and help text

## Step Size Guidelines

- Each step should result in <500 lines of code changed
- Each step should be testable and runnable
- Each step should build incrementally on previous work
- No orphaned or hanging code that isn't integrated

## Integration Points

- MCP tool system registration and discovery
- Git branch parsing logic 
- Issue file system interaction
- Builtin prompt template processing
- Error handling consistency
- Testing patterns

## Benefits of This Approach

1. **Reduced API surface**: Two fewer MCP tools to maintain
2. **Consistent interface**: All issue querying goes through `issue_show`
3. **Simplified mental model**: One tool for showing issues with different behaviors
4. **Maintainability**: Less code duplication, fewer tools to test

This focused approach will complete the remaining specification while maintaining the high quality standards established in the existing codebase.