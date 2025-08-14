# MCP Tool CLI Exclusion Marker Specification

## Overview

This specification defines a metadata attribute system to mark MCP tools that should be excluded from CLI generation. Some MCP tools are designed exclusively for MCP protocol interactions and should not be exposed as CLI commands.

## Problem Statement

Currently, all MCP tools are automatically exposed as CLI commands through the CLI generation system. However, some tools like `issue_work` and `issue_merge` are specifically designed for MCP workflows and should not be available as direct CLI commands because:

1. They expect specific MCP context and state management
2. They use MCP-specific error handling patterns (abort files, workflow termination)
3. They are part of larger MCP workflow orchestrations
4. Direct CLI usage could bypass important workflow validation

## Solution: CLI Exclusion Marker

### Attribute Definition

Define a new attribute `#[cli_exclude]` that can be applied to MCP tool implementations to mark them as excluded from CLI generation.

```rust
#[cli_exclude]
#[derive(Default)]
pub struct IssueWorkTool;

#[cli_exclude] 
#[derive(Default)]
pub struct IssueMergeTool;
```

### Attribute Properties

- **Target**: Applied at the struct level for MCP tool implementations
- **Behavior**: Tools marked with `#[cli_exclude]` are skipped during CLI command generation
- **Scope**: Affects only CLI generation, does not impact MCP tool registration or functionality
- **Documentation**: Should include a comment explaining why the tool is excluded from CLI

### Implementation Requirements

1. **Attribute Processing**: The CLI generation system must detect and respect the `#[cli_exclude]` attribute
2. **Tool Registry**: MCP tool registry should continue to register excluded tools for MCP usage
3. **Documentation**: Generate documentation indicating which tools are MCP-only
4. **Validation**: Ensure excluded tools still undergo normal testing and validation

## Initial Candidates for Exclusion

### `issue_work` Tool
- **Reason**: Designed for MCP workflow state transitions
- **MCP Context**: Requires git operations context and issue storage
- **Workflow Integration**: Part of larger issue management workflows
- **CLI Alternative**: Users should use git commands directly for branch operations

### `issue_merge` Tool  
- **Reason**: Complex merge logic with abort file handling
- **MCP Context**: Requires coordinated state between git ops and issue storage
- **Workflow Integration**: Designed for automated workflow completion
- **CLI Alternative**: Users should use git merge commands directly

## Benefits

1. **Cleaner CLI**: Reduces CLI surface area to user-focused commands
2. **Clearer Intent**: Separates user-facing tools from workflow orchestration tools  
3. **Reduced Confusion**: Prevents users from accidentally invoking workflow-specific tools
4. **Better Documentation**: Clear distinction between CLI and MCP-only tools
5. **Maintainability**: Easier to evolve MCP-specific tools without CLI compatibility concerns

## Implementation Strategy

### Phase 1: Attribute Definition
- Define the `#[cli_exclude]` attribute macro
- Update CLI generation code to detect and skip marked tools
- Add attribute to `issue_work` and `issue_merge` tools

### Phase 2: Documentation Updates  
- Update CLI help text to indicate MCP-only tools
- Document the exclusion system for developers
- Update tool descriptions to clarify usage context

### Phase 3: Validation
- Ensure excluded tools still function correctly in MCP context
- Verify CLI generation skips marked tools
- Test that tool registry continues to register all tools

## Future Considerations

1. **Additional Attributes**: Could extend with `#[mcp_only]`, `#[internal_only]`, etc.
2. **Conditional Inclusion**: Attributes could support conditions like `#[cli_exclude(when = "feature")]`
3. **Tool Categories**: Group tools by usage context (user-facing, workflow, internal, etc.)
4. **Dynamic Exclusion**: Runtime configuration to include/exclude certain tool categories

## Alternative Approaches Considered

### Separate Tool Categories
- **Pros**: More explicit separation of concerns
- **Cons**: Requires restructuring existing tool organization

### Configuration-Based Exclusion
- **Pros**: Runtime flexibility
- **Cons**: Less explicit, harder to track which tools are excluded

### Naming Convention
- **Pros**: No code changes required
- **Cons**: Relies on convention, not enforced

## Conclusion

The `#[cli_exclude]` attribute provides a clean, explicit way to mark MCP tools that should not be exposed as CLI commands. This approach maintains the existing tool architecture while providing clear metadata about tool usage context and intent.