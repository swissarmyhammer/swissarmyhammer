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

### Real-World Integration Example

Here's how the system works in practice:

```rust
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use swissarmyhammer_tools::cli::CliExclusionDetector;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Registry automatically detects exclusions during tool registration
    let mut registry = ToolRegistry::new();
    
    // Register tools - exclusions detected automatically
    registry.register_tool(Box::new(CreateMemoTool::default()))?;     // CLI-eligible
    registry.register_tool(Box::new(IssueWorkTool::default()))?;      // MCP-only
    registry.register_tool(Box::new(IssueMergeTool::default()))?;     // MCP-only
    registry.register_tool(Box::new(AbortCreateTool::default()))?;    // MCP-only
    
    // Get CLI generation metadata
    let detector = registry.as_exclusion_detector();
    
    // Generate CLI commands for eligible tools only
    generate_cli_commands(&detector)?;
    
    // Generate documentation about excluded tools  
    generate_mcp_only_documentation(&detector)?;
    
    Ok(())
}

fn generate_cli_commands<T: CliExclusionDetector>(detector: &T) -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating CLI commands for eligible tools:");
    
    for tool_name in detector.get_cli_eligible_tools() {
        let metadata = detector.get_tool_metadata(&tool_name).unwrap();
        println!("  {} - {}", tool_name, metadata.description.unwrap_or("No description"));
        
        // Generate actual CLI command definition
        generate_clap_command(&tool_name, &metadata)?;
    }
    
    Ok(())
}

fn generate_mcp_only_documentation<T: CliExclusionDetector>(detector: &T) -> Result<(), Box<dyn std::error::Error>> {
    println!("\\nMCP-Only Tools (excluded from CLI):");
    
    for tool_name in detector.get_excluded_tools() {
        let metadata = detector.get_tool_metadata(&tool_name).unwrap();
        println!("  {} - {}", 
            tool_name, 
            metadata.exclusion_reason.unwrap_or("Workflow orchestration tool")
        );
        
        // Generate alternative CLI commands documentation
        if let Some(alternative) = get_cli_alternative(&tool_name) {
            println!("    Alternative: {}", alternative);
        }
    }
    
    Ok(())
}

fn get_cli_alternative(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "issue_work" => Some("git checkout -b issue/name"),
        "issue_merge" => Some("git merge issue/name"),
        "abort_create" => Some("Ctrl+C or kill signal"),
        _ => None,
    }
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

## Developer Guide

### Creating New Tools with Exclusion Awareness

When creating new MCP tools, follow this decision process:

#### Step 1: Analyze Tool Purpose

Ask these key questions:
- **User Intent**: Would a user reasonably want to call this directly?
- **Context Dependency**: Does this require MCP workflow context?
- **Error Patterns**: Does this use abort files for error handling?
- **State Coordination**: Does this coordinate state between systems?

#### Step 2: Implementation Pattern

```rust
// For user-facing tools (CLI + MCP)
/// Tool for creating memos with user-provided content
/// 
/// This tool provides direct value to CLI users and works
/// independently without requiring MCP workflow context.
#[derive(Default)]
pub struct CreateMemoTool;

impl McpTool for CreateMemoTool {
    fn name(&self) -> &'static str { "memo_create" }
    // ... standard implementation
}

// Note: No CliExclusionMarker - defaults to CLI-eligible
```

```rust
// For workflow tools (MCP-only)  
/// Tool for coordinating issue workflow state transitions
///
/// This tool manages git branch operations within issue workflows,
/// uses abort files for error handling, and requires MCP context.
/// CLI users should use: git checkout -b issue/name
#[cli_exclude]
#[derive(Default)]
pub struct IssueWorkTool;

impl McpTool for IssueWorkTool {
    fn name(&self) -> &'static str { "issue_work" }
    // ... implementation with abort file usage
}

impl CliExclusionMarker for IssueWorkTool {
    fn is_cli_excluded(&self) -> bool {
        true
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("MCP workflow state transition - requires MCP context and uses abort patterns")
    }
}
```

#### Step 3: Documentation Standards

Always document the exclusion decision:

```rust
/// # Tool Classification: MCP-Only
///
/// This tool is excluded from CLI generation because:
/// 
/// 1. **MCP Context Dependency**: Requires workflow state coordination
/// 2. **Abort File Patterns**: Uses `create_abort_file_current_dir()` for error handling
/// 3. **Git State Management**: Coordinates complex git operations with issue storage
/// 4. **CLI Alternative Available**: Users should use `git checkout -b issue/name`
///
/// ## Usage in MCP Workflows
/// 
/// ```javascript
/// // Called from Claude Code workflows
/// await tools.issue_work({ name: "feature-xyz" });
/// ```
///
/// ## CLI Alternative
///
/// ```bash
/// # Direct git operations for CLI users
/// git checkout -b issue/feature-xyz
/// ```
#[cli_exclude]
pub struct IssueWorkTool;
```

#### Step 4: Testing Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_tools::cli::CliExclusionMarker;

    #[test]
    fn test_exclusion_configuration() {
        let tool = IssueWorkTool::default();
        
        // Verify exclusion status
        assert!(tool.is_cli_excluded());
        
        // Verify exclusion reason is documented
        let reason = tool.exclusion_reason().unwrap();
        assert!(reason.contains("MCP workflow"));
        assert!(reason.contains("context"));
        
        // Verify tool name matches expectation
        assert_eq!(tool.name(), "issue_work");
    }

    #[test]
    fn test_inclusion_default() {
        let tool = CreateMemoTool::default();
        
        // Default implementation should be CLI-eligible  
        assert!(!tool.is_cli_excluded());
        assert!(tool.exclusion_reason().is_none());
    }

    #[test]
    fn test_registry_integration() {
        let mut registry = ToolRegistry::new();
        registry.register_tool(Box::new(IssueWorkTool::default())).unwrap();
        registry.register_tool(Box::new(CreateMemoTool::default())).unwrap();
        
        let detector = registry.as_exclusion_detector();
        
        // Verify exclusion detection
        assert!(detector.is_cli_excluded("issue_work"));
        assert!(!detector.is_cli_excluded("memo_create"));
        
        // Verify bulk operations
        let excluded = detector.get_excluded_tools();
        let eligible = detector.get_cli_eligible_tools();
        
        assert!(excluded.contains(&"issue_work".to_string()));
        assert!(eligible.contains(&"memo_create".to_string()));
    }
}
```

### Common Patterns and Examples

#### Workflow Orchestration Pattern

```rust
/// Tools that coordinate multiple operations in workflows
#[cli_exclude]
pub struct WorkflowCoordinatorTool {
    // Complex state management
    // Abort file error handling  
    // Multi-system coordination
}

impl CliExclusionMarker for WorkflowCoordinatorTool {
    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("Workflow orchestration requires MCP context for state coordination")
    }
}
```

#### Error Recovery Pattern

```rust  
/// Tools that handle workflow error recovery
#[cli_exclude]
pub struct ErrorRecoveryTool {
    // Abort file creation
    // Workflow termination
    // State rollback
}

impl CliExclusionMarker for ErrorRecoveryTool {
    fn exclusion_reason(&self) -> Option<&'static str> {
        Some("Error recovery tool - uses abort file patterns for workflow termination")
    }
}
```

#### User Content Pattern

```rust
/// Tools that create or manage user content directly
#[derive(Default)]
pub struct ContentManagementTool {
    // Direct user value
    // Independent operation
    // No complex state coordination
}

// No CliExclusionMarker implementation = CLI-eligible by default
```

## Migration Guide

### For Existing Tools

1. **Evaluate Tools**: Use decision criteria to determine if exclusion is appropriate
2. **Add Attribute**: Add `#[sah_marker_macros::cli_exclude]` to struct
3. **Implement Trait**: Implement `CliExclusionMarker` with appropriate reason
4. **Update Documentation**: Document the exclusion decision
5. **Add Tests**: Test exclusion behavior and integration

### Migration Checklist

```markdown
- [ ] Analyzed tool purpose against decision criteria
- [ ] Added #[cli_exclude] attribute if appropriate  
- [ ] Implemented CliExclusionMarker trait with clear reason
- [ ] Updated tool documentation with exclusion rationale
- [ ] Added CLI alternative documentation where applicable
- [ ] Created comprehensive tests for exclusion behavior
- [ ] Verified registry integration works correctly
- [ ] Updated any existing CLI generation to respect exclusions
```

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

## Troubleshooting

### Common Issues

#### Tool Not Detected as Excluded

**Symptoms:**
- Tool appears in CLI-eligible list when it should be excluded
- CLI generation includes tools marked with `#[cli_exclude]`

**Diagnosis:**
```rust
#[test]
fn debug_exclusion_detection() {
    let tool = SuspectedExcludedTool::default();
    
    // Check attribute presence (compile-time verification)
    println!("Tool name: {}", tool.name());
    
    // Check trait implementation
    println!("Is excluded: {}", tool.is_cli_excluded());
    println!("Reason: {:?}", tool.exclusion_reason());
    
    // Check registry integration
    let mut registry = ToolRegistry::new();
    registry.register_tool(Box::new(tool)).unwrap();
    let detector = registry.as_exclusion_detector();
    
    println!("Registry sees as excluded: {}", 
             detector.is_cli_excluded("suspected_tool"));
}
```

**Solutions:**
1. Verify both `#[cli_exclude]` attribute AND `CliExclusionMarker` trait are implemented
2. Ensure trait implementation returns `true` from `is_cli_excluded()`
3. Check tool name matches between trait and registry registration
4. Verify macro is imported: `use sah_marker_macros::cli_exclude;`

#### Inconsistent CLI Generation

**Symptoms:**
- Some excluded tools still appear in generated CLI
- CLI help shows tools that shouldn't be there

**Solutions:**
1. Ensure CLI generation code uses `get_cli_eligible_tools()` not all tools
2. Verify exclusion detector is created from the same registry used in CLI generation
3. Check for cached or stale CLI generation results
4. Validate that CLI generation respects the detector interface

#### Missing Exclusion Reasons

**Solutions:**
```rust
impl CliExclusionMarker for MyTool {
    fn is_cli_excluded(&self) -> bool {
        true
    }

    fn exclusion_reason(&self) -> Option<&'static str> {
        // Always provide a clear, helpful reason
        Some("MCP workflow coordination tool - requires context and uses abort patterns")
    }
}
```

### Best Practices Summary

1. **Always implement both attribute and trait** for excluded tools
2. **Provide clear, actionable exclusion reasons** that help users understand alternatives
3. **Test exclusion behavior comprehensively** including registry integration
4. **Document CLI alternatives** for excluded workflow tools
5. **Use consistent naming patterns** between tool names and exclusion references
6. **Cache exclusion queries** when processing large tool sets
7. **Validate exclusion decisions** against the documented criteria