# Enhance Tool Registry with Exclusion Tracking

Refer to /Users/wballard/github/sah-marker/ideas/marker.md

## Objective

Extend the existing `ToolRegistry` to track and manage CLI exclusion metadata for all registered tools, creating a foundation for future CLI generation systems.

## Implementation Tasks

### 1. Extend ToolRegistry Structure

#### Add Exclusion Metadata Storage
```rust
/// Registry for managing MCP tools with CLI exclusion tracking
#[derive(Default)]
pub struct ToolRegistry {
    /// Internal storage mapping tool names to trait objects
    tools: HashMap<String, Box<dyn McpTool>>,
    
    /// CLI exclusion metadata for each tool
    exclusion_metadata: HashMap<String, ToolCliMetadata>,
}

/// Metadata about tool CLI eligibility
#[derive(Debug, Clone)]
pub struct ToolCliMetadata {
    pub name: String,
    pub is_cli_excluded: bool,
    pub exclusion_reason: Option<String>,
    pub cli_alternatives: Vec<String>,
}
```

### 2. Registration Enhancement

#### Update register() Method
```rust
impl ToolRegistry {
    /// Register a tool in the registry with automatic exclusion detection
    pub fn register<T: McpTool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        
        // Detect CLI exclusion (placeholder for actual detection logic)
        let is_excluded = Self::detect_cli_exclusion::<T>();
        
        let metadata = ToolCliMetadata {
            name: name.clone(),
            is_cli_excluded: is_excluded,
            exclusion_reason: if is_excluded {
                Some("Tool marked with #[cli_exclude] attribute".to_string())
            } else {
                None
            },
            cli_alternatives: Vec::new(),
        };
        
        self.exclusion_metadata.insert(name.clone(), metadata);
        self.tools.insert(name, Box::new(tool));
    }
}
```

### 3. Query Methods for Exclusion

#### Add Exclusion Query APIs
```rust
impl ToolRegistry {
    /// Check if a tool is excluded from CLI generation
    pub fn is_cli_excluded(&self, tool_name: &str) -> bool {
        self.exclusion_metadata
            .get(tool_name)
            .map(|meta| meta.is_cli_excluded)
            .unwrap_or(false)
    }
    
    /// Get all tools marked for CLI exclusion
    pub fn get_excluded_tools(&self) -> Vec<&ToolCliMetadata> {
        self.exclusion_metadata
            .values()
            .filter(|meta| meta.is_cli_excluded)
            .collect()
    }
    
    /// Get all tools eligible for CLI generation
    pub fn get_cli_eligible_tools(&self) -> Vec<&ToolCliMetadata> {
        self.exclusion_metadata
            .values()
            .filter(|meta| !meta.is_cli_excluded)
            .collect()
    }
    
    /// Get CLI metadata for a specific tool
    pub fn get_tool_metadata(&self, tool_name: &str) -> Option<&ToolCliMetadata> {
        self.exclusion_metadata.get(tool_name)
    }
    
    /// List tools by category (excluded vs eligible)
    pub fn list_tools_by_category(&self) -> (Vec<&ToolCliMetadata>, Vec<&ToolCliMetadata>) {
        let mut excluded = Vec::new();
        let mut eligible = Vec::new();
        
        for metadata in self.exclusion_metadata.values() {
            if metadata.is_cli_excluded {
                excluded.push(metadata);
            } else {
                eligible.push(metadata);
            }
        }
        
        (excluded, eligible)
    }
}
```

### 4. Exclusion Detection Logic

#### Placeholder Detection Method  
```rust
impl ToolRegistry {
    /// Detect if a tool has CLI exclusion attribute
    /// 
    /// This is a placeholder implementation. In the future, this could use
    /// procedural macro introspection or build-time analysis to detect
    /// the #[cli_exclude] attribute.
    fn detect_cli_exclusion<T: McpTool>() -> bool {
        // For now, manually track known excluded tools
        let excluded_tools = [
            "issue_work",
            "issue_merge", 
            "abort_create",
        ];
        
        let tool_name = std::any::type_name::<T>();
        excluded_tools.iter().any(|&excluded| {
            tool_name.contains(&excluded.replace('_', ""))
        })
    }
}
```

## Testing Requirements

### 1. Registry Enhancement Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exclusion_tracking() {
        let mut registry = ToolRegistry::new();
        
        registry.register(WorkIssueTool::new()); // Should be excluded
        registry.register(CreateMemoTool::new()); // Should be eligible
        
        assert!(registry.is_cli_excluded("issue_work"));
        assert!(!registry.is_cli_excluded("memo_create"));
        
        let excluded = registry.get_excluded_tools();
        let eligible = registry.get_cli_eligible_tools();
        
        assert_eq!(excluded.len(), 1);
        assert_eq!(eligible.len(), 1);
    }
}
```

### 2. Metadata Accuracy Tests
```rust
#[test]
fn test_metadata_accuracy() {
    let mut registry = ToolRegistry::new();
    registry.register(WorkIssueTool::new());
    
    let metadata = registry.get_tool_metadata("issue_work").unwrap();
    assert_eq!(metadata.name, "issue_work");
    assert!(metadata.is_cli_excluded);
    assert!(metadata.exclusion_reason.is_some());
}
```

### 3. Query Method Tests
- Test all new query methods return correct results
- Verify categorization logic works properly  
- Ensure edge cases are handled correctly

### 4. Backward Compatibility Tests
- Verify existing registry functionality still works
- Test that MCP operations are unaffected
- Ensure tool registration behavior is preserved

## Integration Requirements

### 1. Update All Registration Functions
- Modify `register_issue_tools()` and similar functions
- Ensure all tools get proper metadata tracking
- Maintain existing registration patterns

### 2. Update Tool Listing
```rust
/// Enhanced tool listing with exclusion information
pub fn list_tools_with_metadata(&self) -> Vec<(Tool, &ToolCliMetadata)> {
    self.tools
        .values()
        .filter_map(|tool| {
            let mcp_tool = Tool {
                name: tool.name().into(),
                description: Some(tool.description().into()),
                input_schema: std::sync::Arc::new(/* schema */),
                annotations: None,
            };
            
            self.get_tool_metadata(tool.name())
                .map(|metadata| (mcp_tool, metadata))
        })
        .collect()
}
```

## Documentation Updates

### 1. Registry Documentation
- Update registry rustdoc to explain exclusion tracking
- Document all new methods with examples
- Explain the metadata structure and usage

### 2. Integration Examples
```rust
/// Example: Using registry for CLI generation
/// 
/// ```rust
/// let registry = create_registry();
/// let eligible_tools = registry.get_cli_eligible_tools();
/// 
/// for tool_meta in eligible_tools {
///     println!("CLI-eligible tool: {}", tool_meta.name);
///     generate_cli_command(tool_meta);
/// }
/// ```
```

## Acceptance Criteria

- [ ] Registry tracks exclusion metadata for all tools
- [ ] Query methods provide accurate exclusion information
- [ ] Detection logic correctly identifies excluded tools  
- [ ] Metadata structure captures all necessary information
- [ ] Backward compatibility is maintained
- [ ] Comprehensive tests validate all functionality
- [ ] Documentation explains enhanced registry capabilities

## Notes

This step creates the infrastructure for tracking CLI exclusions within the existing tool registry, preparing for future CLI generation while maintaining full backward compatibility with existing MCP functionality.