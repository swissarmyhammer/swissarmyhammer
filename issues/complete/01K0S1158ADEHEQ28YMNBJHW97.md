Review mcp and cli, make sure that we have not duplicated logic between then. Both mcp and cli should be clients to underlying library code, formatting results, and error handling.

## Proposed Solution

After analyzing the codebase, I've identified significant duplication between CLI and MCP implementations, particularly in issue management functionality. Here's my plan to consolidate the logic:

### Current Duplication Issues

1. **Issue Management**: Both CLI (`swissarmyhammer-cli/src/issue.rs`) and MCP (`swissarmyhammer/src/mcp/tool_handlers.rs`) implement similar create/update/complete logic
2. **Git Branch Operations**: Both implement branch switching and merging independently
3. **Content Input Processing**: CLI has specialized content handling that MCP reimplements differently
4. **Status/Progress Tracking**: Similar but inconsistent status reporting across interfaces
5. **Error Handling**: Different error patterns and responses between CLI and MCP

### Consolidation Plan

1. **Create Core Business Logic Layer**
   - Move common issue operations into `swissarmyhammer/src/issues/utils.rs`
   - Extend `IssueStorage` trait with batch and utility operations
   - Add shared validation and formatting functions

2. **Consolidate Git Operations**
   - Move branch management logic to `swissarmyhammer/src/git.rs`
   - Create shared functions for issue branch creation/merging
   - Standardize git error handling

3. **Shared Input/Content Processing**
   - Extract CLI's content handling into core library
   - Create unified content processing utilities for both interfaces
   - Support file input, stdin, and direct content consistently

4. **Unified Status/Progress System**
   - Create shared status reporting functions
   - Standardize progress tracking across CLI and MCP
   - Consistent error messaging and user feedback

5. **Refactor Interfaces**
   - Update CLI to use consolidated core functions (thin client layer)
   - Update MCP tool handlers to use same core functions (formatting for JSON responses)
   - Both become lightweight adapters to the core library

### Implementation Steps

1. Create shared utility functions in core library
2. Write comprehensive tests for new shared functionality  
3. Refactor CLI to use shared functions (maintaining existing behavior)
4. Refactor MCP to use shared functions (maintaining MCP protocol compliance)
5. Remove duplicate code from both CLI and MCP implementations
6. Ensure consistent error handling and user experience across both interfaces

This approach will reduce code duplication by ~30-40% and ensure consistent behavior between CLI and MCP while maintaining their distinct presentation styles.