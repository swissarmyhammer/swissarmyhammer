# CLI Exclusion System

## Overview

The CLI exclusion system provides infrastructure for marking MCP tools that should be excluded from CLI generation. This system addresses the need to distinguish between user-facing tools and workflow orchestration tools that are designed specifically for MCP protocol interactions.

## Problem Statement

Some MCP tools are designed exclusively for MCP workflow operations and should not be exposed as CLI commands because:

1. **MCP-Specific Context**: They expect specific MCP protocol context and state management
2. **Workflow Orchestration**: They are part of larger MCP workflow orchestrations 
3. **Error Handling Patterns**: They use MCP-specific error handling (abort files, workflow termination)
4. **State Management**: They require coordinated state between multiple systems

Direct CLI usage of these tools could bypass important workflow validation and lead to inconsistent system state.

## Architecture

### Components

The CLI exclusion system uses a trait-based architecture with three main components:

1. **`#[cli_exclude]` Attribute Macro**: Compile-time marker for tools
2. **`CliExclusionMarker` Trait**: Runtime queryable exclusion status
3. **`CliExclusionDetector` Trait**: Interface for CLI generation systems

### Design Philosophy

Rather than attempting complex runtime attribute parsing, this system uses compile-time trait implementations that tools can opt into. This provides:

- **Type Safety**: Compile-time validation of exclusion status
- **Performance**: No runtime reflection or parsing overhead
- **Integration**: Easy integration with existing tool patterns
- **Documentation**: Clear documentation of exclusion rationale

## Usage Patterns

### Marking Tools for Exclusion

Tools that should be excluded from CLI generation use both the attribute and trait:

```rust
use swissarmyhammer_tools::cli::CliExclusionMarker;

/// Tool for switching to work on an issue
/// 
/// This tool is designed for MCP workflow state transitions and should not
/// be exposed as a CLI command since it requires specific MCP context.
#[sah_marker_macros::cli_exclude]
#[derive(Default)]
pub struct WorkIssueTool;

impl CliExclusionMarker for WorkIssueTool {
    fn is_cli_excluded(&self) -> bool {
        true
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("MCP workflow state transition tool - requires MCP context and uses abort file patterns")
    }
}
```

### When to Use CLI Exclusion

**Use `#[cli_exclude]` for:**

- **Workflow Orchestration Tools**: Tools that manage workflow state transitions
  - Example: `issue_work` - manages git branch operations within issue workflows
  - Example: `issue_merge` - coordinates git merges with issue completion

- **Abort Pattern Tools**: Tools that use abort file patterns for workflow termination
  - Tools that call `create_abort_file_current_dir()` for error handling
  - Tools designed to terminate workflows on validation failures

- **State Coordination Tools**: Tools requiring coordination between multiple systems
  - Tools that need both git operations and issue storage consistency
  - Tools that manage complex state transitions across boundaries

**Do NOT use `#[cli_exclude]` for:**

- **User-Facing Operations**: Tools users might want to invoke directly
  - Content creation tools (memos, issues)
  - Search and query operations
  - File operations and information display

- **Standalone Utilities**: Tools that work independently
  - Tools that don't require MCP workflow context
  - Tools that provide direct value to CLI users

### Decision Criteria

Ask these questions when considering CLI exclusion:

1. **Context Dependency**: Does this tool require specific MCP workflow context?
2. **Error Patterns**: Does this tool use abort files or MCP-specific error handling?
3. **State Coordination**: Does this tool coordinate state between multiple systems?
4. **User Intent**: Would a user reasonably want to call this directly from CLI?

If answers to 1-3 are "yes" and 4 is "no", consider using `#[cli_exclude]`.

## Real-World Examples

### Issue Work Tool

```rust
/// Tool for switching to work on an issue
///
/// This tool manages git branch operations within issue workflows,
/// validates branch states, and uses abort files for error handling.
#[sah_marker_macros::cli_exclude]
#[derive(Default)]
pub struct WorkIssueTool;

impl CliExclusionMarker for WorkIssueTool {
    fn is_cli_excluded(&self) -> bool {
        true
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("MCP workflow state transition tool - requires MCP context and uses abort file patterns")
    }
}
```

**Why excluded:**
- Performs complex git branch validation specific to issue workflows
- Uses `create_abort_file_current_dir()` for workflow termination
- Requires coordination between git operations and issue storage
- CLI users should use git commands directly for branch operations

### Issue Merge Tool

```rust
/// Tool for merging an issue work branch
///
/// This tool coordinates merge operations with issue completion,
/// validates workflow state, and handles complex error scenarios.
#[sah_marker_macros::cli_exclude]
#[derive(Default)]
pub struct MergeIssueTool;

impl CliExclusionMarker for MergeIssueTool {
    fn is_cli_excluded(&self) -> bool {
        true
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("MCP workflow orchestration tool - requires coordinated state management and uses abort file patterns")
    }
}
```

**Why excluded:**
- Auto-completes issues before merging (complex state transition)
- Uses abort files for validation failures
- Requires being on specific branch types for safety
- CLI users should use git merge commands directly

### Counter-Example: Memo Creation Tool

```rust
/// Tool for creating new memos
///
/// This is a user-facing tool that provides direct value to CLI users
/// and doesn't require MCP workflow context.
#[derive(Default)]
pub struct CreateMemoTool;

// Note: No CliExclusionMarker implementation - defaults to CLI-eligible
```

**Why NOT excluded:**
- Provides direct value to CLI users
- Works independently without MCP context
- Simple operation without complex state management
- Users would reasonably want to call this from CLI

## Integration with Tool Registry

The CLI exclusion system integrates seamlessly with the existing tool registry:

```rust
use swissarmyhammer_tools::cli::CliExclusionDetector;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;

// Create registry and populate with tools
let registry = ToolRegistry::new();

// Get exclusion detector for CLI generation
let detector = registry.as_exclusion_detector();

// Query exclusion status
let excluded_tools = detector.get_excluded_tools();
let eligible_tools = detector.get_cli_eligible_tools();

// Check specific tool
if detector.is_cli_excluded("issue_work") {
    println!("Tool is excluded from CLI generation");
}
```

### Extension Methods

The registry provides convenient extension methods:

```rust
impl ToolRegistry {
    /// Get a CLI exclusion detector for this registry
    pub fn as_exclusion_detector(&self) -> impl CliExclusionDetector {
        // Returns detector implementation
    }
}
```

## CLI Generation Integration

Future CLI generation systems should use the `CliExclusionDetector` interface:

```rust
use swissarmyhammer_tools::cli::CliExclusionDetector;

fn generate_cli_commands<T: CliExclusionDetector>(detector: &T) {
    // Get all CLI-eligible tools
    let eligible_tools = detector.get_cli_eligible_tools();
    
    for tool_name in eligible_tools {
        // Generate CLI command for this tool
        generate_command_for_tool(&tool_name);
    }
    
    // Optional: Generate documentation about excluded tools
    let excluded_tools = detector.get_excluded_tools();
    if !excluded_tools.is_empty() {
        generate_exclusion_documentation(&excluded_tools);
    }
}
```

### CLI Help Integration

CLI systems can provide information about excluded tools:

```rust
fn show_mcp_only_tools<T: CliExclusionDetector>(detector: &T) {
    let metadata = detector.get_all_tool_metadata();
    
    println!("MCP-Only Tools (not available in CLI):");
    for meta in metadata {
        if meta.is_cli_excluded {
            println!("  {} - {}", 
                meta.name, 
                meta.exclusion_reason.unwrap_or("No reason given")
            );
        }
    }
}
```

## Testing and Validation

### Unit Testing Patterns

Test both the attribute and trait implementations:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_excluded_tool_has_correct_status() {
        let tool = WorkIssueTool::default();
        
        // Test trait implementation
        assert!(tool.is_cli_excluded());
        assert_eq!(
            tool.exclusion_reason().unwrap(),
            "MCP workflow state transition tool - requires MCP context and uses abort file patterns"
        );
    }

    #[test]
    fn test_included_tool_default_behavior() {
        #[derive(Default)]
        struct IncludedTool;
        
        impl CliExclusionMarker for IncludedTool {}
        
        let tool = IncludedTool;
        assert!(!tool.is_cli_excluded());
        assert!(tool.exclusion_reason().is_none());
    }
}
```

### Integration Testing

Test the complete detection system:

```rust
#[test]
fn test_registry_exclusion_detection() {
    let registry = create_test_registry();
    let detector = registry.as_exclusion_detector();
    
    // Test specific exclusions
    assert!(detector.is_cli_excluded("issue_work"));
    assert!(detector.is_cli_excluded("issue_merge"));
    
    // Test inclusions  
    assert!(!detector.is_cli_excluded("memo_create"));
    assert!(!detector.is_cli_excluded("issue_create"));
    
    // Test bulk operations
    let excluded = detector.get_excluded_tools();
    assert!(excluded.contains(&"issue_work".to_string()));
    
    let eligible = detector.get_cli_eligible_tools();
    assert!(eligible.contains(&"memo_create".to_string()));
}
```

## Future Enhancements

### Conditional Exclusion

Future versions might support conditional exclusion:

```rust
// Hypothetical future enhancement
#[cli_exclude(when = "feature = \"workflow-mode\"")]
pub struct ConditionalTool;
```

### Tool Categories

The system could be extended with tool categories:

```rust
// Hypothetical future enhancement
#[derive(Debug, Clone)]
pub enum ToolCategory {
    UserFacing,
    WorkflowOrchestration, 
    Internal,
    Debug,
}
```

### Dynamic Configuration

Runtime configuration might allow inclusion/exclusion overrides:

```rust
// Hypothetical future enhancement
#[cli_exclude(allow_override = true)]
pub struct OverridableTool;
```

## Best Practices

### Documentation Standards

Always document why a tool is excluded:

```rust
/// Tool for complex workflow operation
///
/// This tool is excluded from CLI generation because:
/// 1. It requires specific MCP workflow context
/// 2. It uses abort file patterns for error handling  
/// 3. Users should use standard git commands instead
#[sah_marker_macros::cli_exclude]
#[derive(Default)]
pub struct WorkflowTool;
```

### Error Messages

Provide clear exclusion reasons:

```rust
impl CliExclusionMarker for WorkflowTool {
    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("MCP workflow orchestration only - use git commands for direct branch operations")
    }
}
```

### Testing Coverage

Always test both positive and negative cases:

- Tools that should be excluded are excluded
- Tools that should be included are included  
- Exclusion reasons are accurate and helpful
- Registry integration works correctly

## Migration Guide

### For Existing Tools

1. **Evaluate Tools**: Use decision criteria to determine if exclusion is appropriate
2. **Add Attribute**: Add `#[sah_marker_macros::cli_exclude]` to struct
3. **Implement Trait**: Implement `CliExclusionMarker` with appropriate reason
4. **Update Documentation**: Document the exclusion decision
5. **Add Tests**: Test exclusion behavior and integration

### For CLI Generation Systems

1. **Use Detector Interface**: Query `CliExclusionDetector` instead of assuming all tools are available
2. **Handle Exclusions**: Skip excluded tools during CLI generation
3. **Document Exclusions**: Optionally provide information about MCP-only tools
4. **Test Integration**: Ensure exclusion detection works in your context

## Conclusion

The CLI exclusion system provides a clean, type-safe way to distinguish between user-facing CLI tools and MCP workflow orchestration tools. By using both compile-time attributes and runtime traits, it maintains the existing tool architecture while providing clear metadata about tool usage context and intent.

The system is designed to be:
- **Explicit**: Clear marking and documentation of exclusion decisions
- **Flexible**: Easy to implement and integrate with existing patterns
- **Future-Proof**: Extensible architecture for additional metadata
- **Developer-Friendly**: Good error messages and testing support

Tools should be excluded when they are designed for MCP workflow operations and require specific context that CLI users cannot provide. The decision should be based on the tool's purpose, dependencies, and intended usage patterns.