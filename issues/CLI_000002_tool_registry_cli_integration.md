# Enhance ToolRegistry with CLI Integration Methods

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Extend the ToolRegistry to provide CLI-specific functionality for discovering and categorizing MCP tools for dynamic command generation.

## Implementation Tasks

### 1. Add CLI Discovery Methods to ToolRegistry

Update `swissarmyhammer-tools/src/mcp/tool_registry.rs`:

```rust
impl ToolRegistry {
    // Existing methods unchanged...
    
    /// Get all CLI categories from registered tools
    pub fn get_cli_categories(&self) -> Vec<String> {
        let mut categories = std::collections::HashSet::new();
        
        for tool in self.tools.values() {
            if let Some(category) = tool.cli_category() {
                if !tool.hidden_from_cli() {
                    categories.insert(category.to_string());
                }
            }
        }
        
        let mut result: Vec<String> = categories.into_iter().collect();
        result.sort();
        result
    }
    
    /// Get all tools for a specific CLI category
    pub fn get_tools_for_category(&self, category: &str) -> Vec<&dyn McpTool> {
        self.tools.values()
            .filter(|tool| {
                tool.cli_category() == Some(category) && !tool.hidden_from_cli()
            })
            .map(|tool| tool.as_ref())
            .collect()
    }
    
    /// Get all CLI-visible tools (no category)
    pub fn get_root_cli_tools(&self) -> Vec<&dyn McpTool> {
        self.tools.values()
            .filter(|tool| tool.cli_category().is_none() && !tool.hidden_from_cli())
            .map(|tool| tool.as_ref())
            .collect()
    }
    
    /// Check if a tool exists by CLI path (category/name or just name)
    pub fn get_tool_by_cli_path(&self, cli_path: &str) -> Option<&dyn McpTool> {
        // Handle category/name format
        if let Some((category, name)) = cli_path.split_once('/') {
            return self.get_tools_for_category(category)
                .into_iter()
                .find(|tool| tool.cli_name() == name);
        }
        
        // Handle root-level tools
        self.get_root_cli_tools()
            .into_iter()
            .find(|tool| tool.cli_name() == cli_path)
    }
}
```

### 2. Add CLI Metadata Collection

```rust
#[derive(Debug, Clone)]
pub struct CliToolMetadata {
    pub name: String,
    pub category: Option<String>,
    pub about: Option<String>,
    pub schema: serde_json::Value,
    pub mcp_name: String,
}

impl ToolRegistry {
    /// Collect CLI metadata for all visible tools
    pub fn get_cli_metadata(&self) -> Vec<CliToolMetadata> {
        self.tools.values()
            .filter(|tool| !tool.hidden_from_cli())
            .map(|tool| CliToolMetadata {
                name: tool.cli_name().to_string(),
                category: tool.cli_category().map(|s| s.to_string()),
                about: tool.cli_about().map(|s| s.to_string()),
                schema: tool.schema(),
                mcp_name: tool.name().to_string(),
            })
            .collect()
    }
}
```

### 3. Create Registry Builder Pattern

Add a builder pattern for easier CLI integration:

```rust
pub struct CliRegistryBuilder {
    registry: ToolRegistry,
}

impl CliRegistryBuilder {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }
    
    pub fn categories(&self) -> Vec<String> {
        self.registry.get_cli_categories()
    }
    
    pub fn tools_in_category(&self, category: &str) -> Vec<&dyn McpTool> {
        self.registry.get_tools_for_category(category)
    }
    
    pub fn root_tools(&self) -> Vec<&dyn McpTool> {
        self.registry.get_root_cli_tools()
    }
}
```

### 4. Testing

Add comprehensive tests for new functionality:

```rust
#[cfg(test)]
mod cli_integration_tests {
    use super::*;
    
    #[test]
    fn test_get_cli_categories() {
        let registry = create_test_registry();
        let categories = registry.get_cli_categories();
        
        assert!(categories.contains(&"issue".to_string()));
        assert!(categories.contains(&"memo".to_string()));
        assert_eq!(categories, categories.iter().cloned().sorted().collect::<Vec<_>>());
    }
    
    #[test] 
    fn test_get_tools_for_category() {
        let registry = create_test_registry();
        let issue_tools = registry.get_tools_for_category("issue");
        
        assert!(!issue_tools.is_empty());
        assert!(issue_tools.iter().all(|tool| tool.cli_category() == Some("issue")));
    }
    
    #[test]
    fn test_hidden_tools_excluded() {
        let registry = create_test_registry();
        let all_tools = registry.get_cli_metadata();
        
        assert!(all_tools.iter().all(|metadata| {
            let tool = registry.get_tool(&metadata.mcp_name).unwrap();
            !tool.hidden_from_cli()
        }));
    }
}
```

## Success Criteria

- [ ] ToolRegistry has CLI discovery methods
- [ ] get_cli_categories() returns sorted category list
- [ ] get_tools_for_category() filters correctly
- [ ] get_tool_by_cli_path() supports category/name lookup
- [ ] CliToolMetadata struct captures necessary information
- [ ] Hidden tools are properly excluded
- [ ] Comprehensive test coverage for all new methods
- [ ] Integration with existing tool registration works

## Architecture Notes

- Builds on trait extensions from previous step
- Provides foundation for dynamic command generation
- Maintains separation of concerns between MCP and CLI layers
- Follows existing registry patterns in codebase