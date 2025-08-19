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