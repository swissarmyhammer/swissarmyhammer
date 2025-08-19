# Create Attribute Detection Utilities

Refer to /Users/wballard/github/sah-marker/ideas/marker.md

## Objective

Build utilities to detect and process the `#[cli_exclude]` attribute from MCP tool definitions, creating the foundation for CLI generation systems to identify excluded tools.

## Implementation Tasks

### 1. Create Detection Module
- Create `swissarmyhammer-tools/src/cli/attribute_detection.rs`
- Implement parsing utilities to detect `cli_exclude` attributes
- Create trait/interface for attribute querying

### 2. Parsing Infrastructure
```rust
/// Trait for detecting CLI exclusion attributes on MCP tools
pub trait CliExclusionDetector {
    /// Check if a tool type has the cli_exclude attribute
    fn is_cli_excluded(&self, tool_name: &str) -> bool;
    
    /// Get all tools marked for CLI exclusion
    fn get_excluded_tools(&self) -> Vec<String>;
    
    /// Get all tools eligible for CLI generation
    fn get_cli_eligible_tools(&self) -> Vec<String>;
}
```

### 3. Registry Integration
- Extend the existing `ToolRegistry` to track exclusion metadata
- Add exclusion detection during tool registration
- Store exclusion state alongside tool information

### 4. Metadata Storage
```rust
/// Metadata about tool CLI eligibility
#[derive(Debug, Clone)]
pub struct ToolCliMetadata {
    pub name: String,
    pub is_cli_excluded: bool,
    pub exclusion_reason: Option<String>,
}
```

## Testing Requirements

### 1. Attribute Detection Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[cli_exclude]
    struct ExcludedTool;
    
    struct IncludedTool;
    
    #[test]
    fn test_exclusion_detection() {
        let detector = create_test_detector();
        assert!(detector.is_cli_excluded("ExcludedTool"));
        assert!(!detector.is_cli_excluded("IncludedTool"));
    }
}
```

### 2. Registry Integration Tests
- Test that excluded tools are properly tracked in the registry
- Verify registry can list excluded vs eligible tools
- Ensure exclusion detection doesn't affect MCP functionality

### 3. Metadata Tests
- Validate metadata structure and serialization
- Test metadata persistence and retrieval
- Verify metadata accuracy across tool types

## Documentation

### 1. API Documentation
- Document all detection utilities with comprehensive rustdoc
- Include usage examples for detection functions
- Explain the detection algorithms and limitations

### 2. Architecture Documentation
- Document how exclusion detection fits into the tool registry
- Explain the metadata storage and retrieval patterns
- Provide guidance for future CLI generation implementation

## Integration Points

### 1. Tool Registry Enhancement
- Modify `ToolRegistry::register()` to detect and store exclusion metadata
- Add new methods for querying exclusion status
- Maintain backward compatibility with existing registry usage

### 2. Future CLI Generation Preparation
- Design detection utilities to be consumed by CLI generation systems
- Create clear APIs for exclusion queries
- Ensure efficient lookup of exclusion status

## Acceptance Criteria

- [ ] Attribute detection utilities are implemented and tested
- [ ] Registry tracks exclusion metadata for all tools
- [ ] Detection APIs provide easy exclusion status querying
- [ ] Comprehensive tests validate detection accuracy
- [ ] Documentation explains detection algorithms and usage
- [ ] Integration with existing registry is seamless and backward compatible

## Notes

This step creates the infrastructure to programmatically detect which tools are marked for CLI exclusion, preparing for future CLI generation while not disrupting existing MCP functionality.

## Proposed Solution

After examining the codebase architecture, I will implement the attribute detection utilities using the following approach:

### 1. Create CLI Module Structure
- Add `src/cli/mod.rs` to organize CLI-related functionality
- Create `src/cli/attribute_detection.rs` with detection utilities
- Integrate with existing tool registry system

### 2. Implementation Strategy

Rather than trying to detect attributes through runtime reflection (which is very complex in Rust), I'll use a trait-based approach where tools can opt-in to providing their CLI exclusion status:

```rust
/// Trait for tools that can report their CLI exclusion status
pub trait CliExclusionMarker: Send + Sync {
    /// Returns true if this tool should be excluded from CLI generation
    fn is_cli_excluded(&self) -> bool {
        false // Default to included unless explicitly marked
    }
    
    /// Returns an optional reason for CLI exclusion
    fn exclusion_reason(&self) -> Option<&'static str> {
        None
    }
}
```

### 3. Registry Integration
- Extend `ToolRegistry` to track exclusion metadata during registration
- Add methods to query excluded vs CLI-eligible tools
- Store metadata efficiently without breaking existing functionality

### 4. Metadata Storage
- Create `ToolCliMetadata` struct as specified
- Collect exclusion information during tool registration
- Provide efficient lookup and filtering methods

### 5. Benefits of This Approach
- No runtime attribute parsing or reflection needed
- Type-safe and compile-time validated
- Easy to implement and test
- Backward compatible with existing tools
- Follows existing Rust patterns in the codebase

This approach creates the infrastructure needed for future CLI generation while being practical to implement and maintain within Rust's type system constraints.
## Implementation Complete

### What Was Implemented

✅ **CLI Module Structure**: Created `src/cli/mod.rs` and `src/cli/attribute_detection.rs`

✅ **Core Traits and Types**:
- `CliExclusionMarker` trait for tools to declare their CLI exclusion status
- `ToolCliMetadata` struct for storing exclusion information
- `CliExclusionDetector` trait interface for querying exclusion status
- `RegistryCliExclusionDetector` implementation

✅ **ToolRegistry Integration**: Extended with methods:
- `create_cli_exclusion_detector()` - Creates detector from registry
- `get_excluded_tool_names()` - Convenience method for excluded tools
- `get_cli_eligible_tool_names()` - Convenience method for CLI-eligible tools

✅ **McpTool Trait Enhancement**: Added `as_any()` method for downcasting support

✅ **Applied Exclusions**: Added `#[cli_exclude]` attribute and `CliExclusionMarker` implementations to:
- `issue_work` tool - "MCP workflow state transition tool"
- `issue_merge` tool - "MCP workflow orchestration tool"

✅ **Comprehensive Testing**: 13 tests covering:
- Unit tests for all core functionality
- Integration tests with real registry and tools
- Edge cases and error conditions

✅ **Documentation**: Extensive rustdoc with examples and usage patterns

### Key Design Decisions

1. **Trait-Based Detection**: Used trait implementation rather than runtime attribute parsing for type safety and performance

2. **Registry Integration**: Seamlessly integrated with existing `ToolRegistry` without breaking changes

3. **Metadata Caching**: Efficient lookup with cached metadata for performance

4. **Backward Compatibility**: All existing tools work unchanged; exclusion is opt-in

### Usage for CLI Generation

```rust
let mut registry = ToolRegistry::new();
// Register tools including some with #[cli_exclude]

let detector = registry.create_cli_exclusion_detector();
let cli_tools = detector.get_cli_eligible_tools(); // Use these for CLI generation
let mcp_only = detector.get_excluded_tools(); // Document these as MCP-only
```

The implementation successfully creates the foundation for CLI generation systems while maintaining the existing MCP tool architecture.