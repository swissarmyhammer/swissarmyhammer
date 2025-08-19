# Mark Workflow-Specific Tools with CLI Exclusion

Refer to /Users/wballard/github/sah-marker/ideas/marker.md

## Objective

Apply the `#[cli_exclude]` attribute to MCP tools that are specifically designed for workflow operations and should not be exposed as direct CLI commands.

## Target Tools for Exclusion

### 1. Issue Workflow Tools

#### `issue_work` Tool
- **Location**: `swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`
- **Reason**: Designed for MCP workflow state transitions, requires git context
- **Exclusion Logic**: Branch operations with abort file handling, not suitable for direct CLI use

#### `issue_merge` Tool  
- **Location**: `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`
- **Reason**: Complex merge logic with coordinated state management
- **Exclusion Logic**: Requires issue workflow context, handles abort files, auto-completion logic

### 2. Abort System Tool

#### `abort_create` Tool
- **Location**: `swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs`
- **Reason**: Internal workflow termination mechanism
- **Exclusion Logic**: Designed for MCP workflow error handling, not direct user operations

## Implementation Tasks

### 1. Apply Attributes to Target Tools

```rust
// In issues/work/mod.rs
#[cli_exclude]
#[derive(Default)]
pub struct WorkIssueTool;

// In issues/merge/mod.rs  
#[cli_exclude]
#[derive(Default)]
pub struct MergeIssueTool;

// In abort/create/mod.rs
#[cli_exclude]
#[derive(Default)]
pub struct CreateAbortTool;
```

### 2. Add Exclusion Documentation
- Add rustdoc comments explaining why each tool is excluded
- Include references to CLI alternatives where applicable
- Document the workflow context these tools require

### 3. Update Tool Descriptions
```rust
/// Tool for switching to work on an issue (MCP workflow only)
///
/// This tool is designed for MCP workflow operations and coordinates
/// git branch operations within issue management workflows. It requires
/// workflow context and state management not available in direct CLI usage.
///
/// For direct git operations, use standard git commands:
/// ```bash
/// git checkout -b issue/my-issue
/// ```
#[cli_exclude]
#[derive(Default)]
pub struct WorkIssueTool;
```

## Testing Requirements

### 1. Compilation Tests
- Verify all marked tools compile correctly with the attribute
- Ensure attributes don't interfere with existing functionality
- Test that MCP operations still work correctly

### 2. Attribute Detection Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::attribute_detection::CliExclusionDetector;

    #[test]
    fn test_workflow_tools_excluded() {
        let detector = create_detector();
        assert!(detector.is_cli_excluded("issue_work"));
        assert!(detector.is_cli_excluded("issue_merge"));
        assert!(detector.is_cli_excluded("abort_create"));
    }
}
```

### 3. Registry Integration Tests  
- Verify excluded tools are properly tracked in registry
- Test that MCP functionality remains intact
- Ensure exclusion metadata is correct

### 4. MCP Functionality Tests
- Run existing MCP tests to verify tools still work
- Test workflow scenarios to ensure functionality is preserved
- Validate error handling and abort mechanisms

## Documentation Updates

### 1. Tool-Specific Documentation
- Update each tool's description to explain CLI exclusion
- Provide CLI alternatives where applicable
- Document the MCP workflow context these tools require

### 2. Tool Registry Documentation
```rust
/// Register all issue-related tools with the registry
/// 
/// Note: Some issue tools are marked with #[cli_exclude] as they are
/// designed specifically for MCP workflow operations:
/// - issue_work: Git branch operations within workflows
/// - issue_merge: Complex merge operations with state coordination
pub fn register_issue_tools(registry: &mut ToolRegistry) {
    registry.register(CreateIssueTool::new());
    registry.register(WorkIssueTool::new());      // CLI excluded
    registry.register(MergeIssueTool::new());     // CLI excluded
    // ... other tools
}
```

## Validation Steps

### 1. Verify Exclusion Accuracy
- Review each marked tool to confirm exclusion is appropriate  
- Ensure user-facing tools are NOT excluded
- Validate exclusion reasoning documentation

### 2. Test MCP Integration
- Run full MCP test suite to verify functionality
- Test workflow scenarios with excluded tools
- Ensure exclusion doesn't break tool registration

### 3. Documentation Review
- Verify exclusion reasoning is clearly documented
- Ensure CLI alternatives are provided where relevant
- Review that workflow context requirements are explained

## Acceptance Criteria

- [ ] All workflow-specific tools are marked with `#[cli_exclude]`
- [ ] Marked tools compile and function correctly in MCP context
- [ ] Tool descriptions explain exclusion rationale
- [ ] CLI alternatives are documented where applicable
- [ ] Registry properly tracks exclusion metadata
- [ ] Comprehensive tests validate exclusion detection
- [ ] MCP functionality remains intact for all tools

## Notes

This step implements the primary use case for the CLI exclusion system by marking the specific tools identified in the specification that should not be exposed as CLI commands due to their workflow-specific nature.
## Proposed Solution

I will implement the CLI exclusion marking by:

1. **Applying the `#[cli_exclude]` Attribute**: Adding the `#[sah_marker_macros::cli_exclude]` attribute to the three target tools:
   - `issue_work` tool: Handles git branch operations within workflow context
   - `issue_merge` tool: Manages complex merge operations with state coordination
   - `abort_create` tool: Internal workflow termination mechanism

2. **Implementing the CliExclusionMarker Trait**: Each marked tool will implement the `CliExclusionMarker` trait with the required `is_cli_excluded()` method returning `true`.

3. **Adding Comprehensive Documentation**: Enhanced rustdoc comments explaining:
   - Why each tool is excluded from CLI
   - The workflow context these tools require
   - CLI alternatives where applicable
   - References to the exclusion system documentation

4. **Following Established Patterns**: Using the same implementation pattern already established in the codebase, as I can see from the grep results that this system is already partially implemented.

5. **Testing Strategy**: Running compilation tests and MCP functionality tests to ensure the exclusions work correctly without breaking existing functionality.

The implementation will be consistent with the established CLI exclusion system architecture and follow the existing code patterns in the repository.
## Implementation Completed

I have successfully implemented the CLI exclusion marking for all target workflow tools:

### ✅ Tools Updated

1. **`issue_work` Tool** (already had exclusion): `/swissarmyhammer-tools/src/mcp/tools/issues/work/mod.rs`
   - Attribute: `#[sah_marker_macros::cli_exclude]` 
   - Trait: `CliExclusionMarker` implemented
   - Reason: "MCP workflow state transition tool - requires MCP context and uses abort file patterns"
   - Documentation: Comprehensive rustdoc explaining why it's excluded and CLI alternatives

2. **`issue_merge` Tool** (already had exclusion): `/swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`
   - Attribute: `#[sah_marker_macros::cli_exclude]`
   - Trait: `CliExclusionMarker` implemented  
   - Reason: "MCP workflow orchestration tool - requires coordinated state management and uses abort file patterns"
   - Documentation: Clear explanation of workflow context requirements

3. **`abort_create` Tool** (newly added exclusion): `/swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs`
   - Attribute: `#[sah_marker_macros::cli_exclude]` ✅ ADDED
   - Trait: `CliExclusionMarker` implemented ✅ ADDED
   - Reason: "MCP workflow error handling tool - creates internal abort state files for workflow coordination"
   - Documentation: Enhanced with CLI exclusion explanation and alternatives

### ✅ Testing Results

- **Compilation**: ✅ All tools compile successfully with exclusion attributes
- **MCP Functionality**: ✅ All excluded tools still function correctly in MCP context
- **CLI Exclusion Detection**: ✅ Registry correctly identifies excluded tools
- **Integration Tests**: ✅ All CLI integration tests pass, including `test_registry_detects_excluded_issue_tools`

### ✅ Code Quality

- All changes follow established patterns in the codebase
- Documentation is comprehensive and explains exclusion rationale
- CLI alternatives are provided where applicable
- Error handling and functionality remain intact
- Tests verify exclusion detection works correctly

The implementation follows the established CLI exclusion system architecture and ensures that workflow-specific tools are properly marked for MCP-only usage while maintaining their full functionality within the workflow context.