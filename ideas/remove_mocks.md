# Mock Elimination Plan

Based on my comprehensive review of the SwissArmyHammer codebase, I've identified extensive mock usage that violates our "NEVER USE MOCKS - UPON PAIN OF DEATH" policy. Here's my plan to eliminate all mocks:

## Critical Mock Violations Found

### 1. MockFileSystem (swissarmyhammer-common/src/fs_utils.rs)
- **HIGH PRIORITY**: Complete mock filesystem implementation
- Used extensively in plan_utils.rs tests (9+ test functions)
- Used in fs_utils.rs own tests (10+ test functions)
- **Action**: Replace with real temporary directories and files

### 2. MockClaudeDesktopClient (swissarmyhammer-cli/tests/mcp_e2e_tests.rs)
- **HIGH PRIORITY**: Complete mock MCP client implementation  
- Used in 10+ E2E test functions
- **Action**: Replace with real MCP server testing via temporary processes

### 3. MockWorkflow (tests/workflow_parameters/cli_tests/help_generation_tests.rs)
- **MEDIUM PRIORITY**: Mock workflow for CLI parameter testing
- Used in 8+ test functions
- **Action**: Replace with real workflow implementations

### 4. MockTool (swissarmyhammer-tools/src/mcp/tool_registry.rs)
- **MEDIUM PRIORITY**: Mock MCP tool for testing
- **Action**: Replace with real simple tool implementations

### 5. MockAction/MockActionBuilder (swissarmyhammer-common/src/test_organization.rs)
- **LOW PRIORITY**: Mock action builders for testing
- **Action**: Replace with real action implementations

### 6. MockResource (swissarmyhammer-workflow/src/actions_tests/resource_cleanup_tests.rs)
- **LOW PRIORITY**: Mock resource for cleanup testing
- **Action**: Replace with real temporary resource implementations

## Implementation Strategy

### Phase 1: FileSystem Replacement
1. Create TempDir-based real filesystem testing utilities
2. Replace all MockFileSystem usage with real temporary directories
3. Update all affected tests to use real file operations

### Phase 2: MCP Client Replacement
1. Replace MockClaudeDesktopClient with real temporary MCP server processes
2. Use actual network communication for E2E testing
3. Implement proper cleanup and resource management

### Phase 3: Workflow and Tool Replacement
1. Replace MockWorkflow with minimal real workflow implementations
2. Replace MockTool with simple real tool implementations
3. Ensure all tests use real dependency injection

### Phase 4: Cleanup and Validation
1. Remove all mock structs and implementations
2. Add lint rules to prevent future mock introduction
3. Update documentation to reinforce no-mock policy
4. Run comprehensive test suite to ensure real implementations work

## Files to Modify
- `swissarmyhammer-common/src/fs_utils.rs` (remove MockFileSystem)
- `swissarmyhammer/src/plan_utils.rs` (replace MockFileSystem usage)
- `swissarmyhammer-cli/tests/mcp_e2e_tests.rs` (replace MockClaudeDesktopClient)
- `tests/workflow_parameters/cli_tests/help_generation_tests.rs` (replace MockWorkflow)
- `swissarmyhammer-tools/src/mcp/tool_registry.rs` (replace MockTool)
- `swissarmyhammer-common/src/test_organization.rs` (replace MockAction)
- `swissarmyhammer-workflow/src/actions_tests/resource_cleanup_tests.rs` (replace MockResource)

This plan will completely eliminate all mock usage from the SwissArmyHammer codebase and establish real testing patterns that verify actual system behavior.