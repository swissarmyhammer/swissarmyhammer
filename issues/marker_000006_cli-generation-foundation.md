# Create CLI Generation System Foundation

Refer to /Users/wballard/github/sah-marker/ideas/marker.md

## Objective

Build the foundational infrastructure for future CLI generation from MCP tools, respecting the CLI exclusion markers and creating a framework that can generate CLI commands automatically.

## Implementation Tasks

### 1. Create CLI Generation Module

#### Foundation Structure
```rust
// swissarmyhammer-cli/src/generation/mod.rs

/// CLI generation system for MCP tools
pub mod cli_generator;
pub mod command_builder;
pub mod attribute_parser;

/// Re-exports
pub use cli_generator::CliGenerator;
pub use command_builder::CommandBuilder;
```

### 2. CLI Generator Core

#### CliGenerator Implementation
```rust
/// Generates CLI commands from MCP tool definitions
pub struct CliGenerator {
    registry: Arc<ToolRegistry>,
    config: GenerationConfig,
}

impl CliGenerator {
    /// Create a new CLI generator
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            config: GenerationConfig::default(),
        }
    }
    
    /// Generate CLI commands for all eligible tools
    pub fn generate_commands(&self) -> Result<Vec<GeneratedCommand>, GenerationError> {
        let eligible_tools = self.registry.get_cli_eligible_tools();
        
        eligible_tools.into_iter()
            .map(|metadata| self.generate_command_for_tool(metadata))
            .collect()
    }
    
    /// Generate CLI command for a specific tool
    fn generate_command_for_tool(&self, metadata: &ToolCliMetadata) -> Result<GeneratedCommand, GenerationError> {
        let tool = self.registry.get_tool(&metadata.name)
            .ok_or_else(|| GenerationError::ToolNotFound(metadata.name.clone()))?;
            
        CommandBuilder::new()
            .from_mcp_tool(tool)
            .with_metadata(metadata)
            .build()
    }
}
```

### 3. Command Builder

#### Command Structure Generation
```rust
/// Builds CLI command structures from MCP tool definitions
pub struct CommandBuilder {
    name: Option<String>,
    description: Option<String>,
    arguments: Vec<CliArgument>,
    options: Vec<CliOption>,
}

impl CommandBuilder {
    /// Create command from MCP tool definition
    pub fn from_mcp_tool(mut self, tool: &dyn McpTool) -> Self {
        self.name = Some(tool.name().to_string());
        self.description = Some(tool.description().to_string());
        
        // Parse schema to extract arguments and options
        self.parse_schema(tool.schema())
    }
    
    /// Parse JSON schema into CLI arguments
    fn parse_schema(&mut self, schema: serde_json::Value) -> Result<(), ParseError> {
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            for (name, prop) in properties {
                let arg = self.create_argument_from_property(name, prop)?;
                self.arguments.push(arg);
            }
        }
        Ok(())
    }
    
    /// Create CLI argument from JSON schema property
    fn create_argument_from_property(&self, name: &str, property: &serde_json::Value) -> Result<CliArgument, ParseError> {
        let arg_type = property.get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
            
        let description = property.get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");
            
        Ok(CliArgument {
            name: name.to_string(),
            arg_type: arg_type.to_string(),
            description: description.to_string(),
            required: self.is_required_property(name),
        })
    }
}
```

### 4. Generated Command Structure

#### Command Representation
```rust
/// Represents a generated CLI command
#[derive(Debug, Clone)]
pub struct GeneratedCommand {
    pub name: String,
    pub description: String,
    pub arguments: Vec<CliArgument>,
    pub options: Vec<CliOption>,
    pub subcommand_of: Option<String>,
}

/// CLI argument representation
#[derive(Debug, Clone)]
pub struct CliArgument {
    pub name: String,
    pub arg_type: String,
    pub description: String,
    pub required: bool,
}

/// CLI option representation  
#[derive(Debug, Clone)]
pub struct CliOption {
    pub name: String,
    pub short: Option<char>,
    pub long: String,
    pub description: String,
    pub default_value: Option<String>,
}
```

### 5. Configuration System

#### Generation Configuration
```rust
/// Configuration for CLI generation behavior
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    /// Prefix for generated commands (e.g., "sah-")
    pub command_prefix: Option<String>,
    
    /// Whether to generate subcommands or top-level commands
    pub use_subcommands: bool,
    
    /// Custom naming transformations
    pub naming_strategy: NamingStrategy,
    
    /// Whether to include excluded tools (for debugging)
    pub include_excluded: bool,
}

#[derive(Debug, Clone)]
pub enum NamingStrategy {
    /// Keep original tool names (issue_create -> issue-create)
    KeepOriginal,
    /// Group by domain (issue_create -> issue create)
    GroupByDomain,
    /// Flatten all commands (issue_create -> create-issue)
    Flatten,
}
```

## Testing Requirements

### 1. Generation Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_command_generation() {
        let registry = create_test_registry();
        let generator = CliGenerator::new(Arc::new(registry));
        
        let commands = generator.generate_commands().unwrap();
        
        // Should not include excluded tools
        assert!(!commands.iter().any(|cmd| cmd.name == "issue_work"));
        assert!(!commands.iter().any(|cmd| cmd.name == "issue_merge"));
        
        // Should include eligible tools
        assert!(commands.iter().any(|cmd| cmd.name == "memo_create"));
        assert!(commands.iter().any(|cmd| cmd.name == "issue_create"));
    }
}
```

### 2. Schema Parsing Tests
```rust
#[test]
fn test_schema_parsing() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "description": "The title"
            },
            "count": {
                "type": "integer",
                "description": "The count"
            }
        },
        "required": ["title"]
    });
    
    let command = CommandBuilder::new()
        .parse_schema(schema)
        .unwrap()
        .build()
        .unwrap();
    
    assert_eq!(command.arguments.len(), 2);
    assert!(command.arguments.iter().any(|arg| arg.name == "title" && arg.required));
    assert!(command.arguments.iter().any(|arg| arg.name == "count" && !arg.required));
}
```

### 3. Integration Tests
- Test full generation pipeline from registry to commands
- Verify exclusion system integration
- Test various schema formats and edge cases

### 4. Configuration Tests
- Test different naming strategies
- Verify configuration options work correctly
- Test generation with various settings

## Integration Points

### 1. Registry Integration
- Use existing registry exclusion tracking
- Respect tool metadata and exclusion flags
- Maintain compatibility with MCP operations

### 2. Future CLI Integration
- Design for easy integration with existing CLI structure
- Create hooks for command registration
- Prepare for automated CLI updates

### 3. Tool Schema Integration
- Parse existing MCP tool schemas
- Handle schema variations and edge cases
- Support complex parameter types

## Documentation

### 1. Generation System Documentation
- Document the generation pipeline and architecture
- Explain configuration options and strategies
- Provide examples of generated commands

### 2. Integration Guide
- Guide for integrating generated commands with CLI
- Examples of customizing generation behavior
- Troubleshooting guide for generation issues

### 3. Developer Guide
```rust
/// # CLI Generation System
///
/// The CLI generation system automatically creates CLI commands from MCP tool
/// definitions, respecting exclusion markers and providing flexible configuration.
///
/// ## Basic Usage
///
/// ```rust
/// let registry = create_registry();
/// let generator = CliGenerator::new(Arc::new(registry));
/// let commands = generator.generate_commands()?;
/// 
/// for command in commands {
///     println!("Generated: {}", command.name);
/// }
/// ```
```

## Acceptance Criteria

- [ ] CLI generation foundation is implemented and tested
- [ ] Generator respects CLI exclusion markers correctly
- [ ] Schema parsing handles common MCP tool patterns
- [ ] Generated commands have proper structure and metadata
- [ ] Configuration system provides flexible generation options
- [ ] Comprehensive tests validate generation accuracy
- [ ] Documentation explains system architecture and usage

## Notes

This step creates the foundation for automated CLI generation while respecting the exclusion system. It doesn't immediately integrate with the existing CLI but provides the infrastructure for future automated CLI updates.

## Proposed Solution

After examining the existing codebase, I can see that:

1. **ToolRegistry Integration**: The `ToolRegistry` in `swissarmyhammer-tools/src/mcp/tool_registry.rs` already has comprehensive CLI exclusion tracking implemented with methods like `get_cli_eligible_tools()`, `is_cli_excluded()`, etc.

2. **CLI Structure**: The CLI is organized in `swissarmyhammer-cli/src/` with individual command modules like `memo.rs`, `issue.rs`, etc.

3. **MCP Tool Interface**: All tools implement the `McpTool` trait with `name()`, `description()`, and `schema()` methods that provide the metadata needed for CLI generation.

### Implementation Plan

I'll create the CLI generation foundation with these components:

1. **Generation Module**: `swissarmyhammer-cli/src/generation/mod.rs` with sub-modules:
   - `cli_generator.rs` - Core generation logic
   - `command_builder.rs` - Schema to CLI argument parsing
   - `types.rs` - Data structures for generated commands

2. **CliGenerator**: Main generator that uses the existing `ToolRegistry` to:
   - Get CLI-eligible tools using `registry.get_cli_eligible_tools()`
   - Parse each tool's JSON schema into CLI arguments/options
   - Generate structured command representations

3. **CommandBuilder**: Schema parser that converts JSON Schema from `tool.schema()` into:
   - CLI arguments (required parameters)
   - CLI options (optional parameters with defaults)
   - Help text from descriptions

4. **Configuration System**: `GenerationConfig` with naming strategies:
   - `KeepOriginal` - `issue_create` → `issue-create`
   - `GroupByDomain` - `issue_create` → `issue create`
   - `Flatten` - `issue_create` → `create-issue`

5. **Testing**: Comprehensive unit tests for:
   - CLI exclusion respect
   - Schema parsing accuracy
   - Command structure generation
   - Configuration options

This design leverages the existing CLI exclusion infrastructure and provides a clean foundation for future automated CLI generation while maintaining backward compatibility.

## Implementation Complete

✅ **All implementation tasks completed successfully!**

### What Was Implemented

1. **CLI Generation Module Structure**: Created `swissarmyhammer-cli/src/generation/` with organized sub-modules:
   - `mod.rs` - Module entry point and re-exports
   - `types.rs` - Core data structures and configuration
   - `cli_generator.rs` - Main generation orchestrator 
   - `command_builder.rs` - Schema parsing and CLI structure building

2. **Core Components**:

   **CliGenerator**: Main orchestrator that:
   - Integrates with existing `ToolRegistry` CLI exclusion tracking
   - Respects CLI exclusion markers automatically
   - Supports configurable generation strategies
   - Enforces safety limits (max 1000 commands by default)
   - Handles error recovery gracefully

   **CommandBuilder**: Schema parser that:
   - Converts JSON Schema from MCP tools to CLI argument structures
   - Supports all common schema patterns (strings, integers, booleans, enums)
   - Handles validation constraints (minLength, pattern, minimum, etc.)
   - Intelligently categorizes parameters as arguments vs options
   - Applies naming transformations (underscores to dashes, etc.)

   **Data Structures**: Comprehensive type system including:
   - `GeneratedCommand` - Complete command representation
   - `CliArgument` - Required/optional command arguments with constraints
   - `CliOption` - Command-line flags and options with type information
   - `GenerationConfig` - Flexible configuration with multiple naming strategies

3. **Configuration System**: 
   - `NamingStrategy::KeepOriginal` - `issue_create` → `issue-create`
   - `NamingStrategy::GroupByDomain` - `issue_create` → `issue create` (subcommands)
   - `NamingStrategy::Flatten` - `issue_create` → `create-issue`
   - Support for command prefixes, subcommand organization, completion generation

4. **Integration with Existing Systems**:
   - ✅ Leverages existing CLI exclusion infrastructure from `ToolRegistry`
   - ✅ Respects `issue_work`, `issue_merge`, `abort_create` exclusions automatically
   - ✅ Maintains full backward compatibility with MCP operations
   - ✅ Added `async-trait` dependency to workspace configuration

5. **Comprehensive Testing**: All 25 tests pass, covering:
   - CLI exclusion detection and respect
   - Schema parsing accuracy for various JSON Schema patterns  
   - Command structure generation and validation
   - Configuration options and naming strategies
   - Error handling and edge cases
   - Integration with tool registry systems

### Usage Example

```rust
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, NamingStrategy};
use swissarmyhammer_tools::ToolRegistry;
use std::sync::Arc;

// Create registry with tools (existing pattern)
let mut registry = ToolRegistry::new();
// Tools are automatically registered with CLI exclusion detection

// Generate CLI commands
let generator = CliGenerator::new(Arc::new(registry));
let commands = generator.generate_commands().unwrap();

// With custom configuration  
let config = GenerationConfig {
    naming_strategy: NamingStrategy::GroupByDomain,
    use_subcommands: true,
    command_prefix: Some("sah-".to_string()),
    ..Default::default()
};

let generator = generator.with_config(config);
let commands = generator.generate_commands().unwrap();
```

### Key Achievements

- **Zero Breaking Changes**: All existing code continues to work unchanged
- **Automatic CLI Exclusion**: Respects existing exclusion markers without manual configuration
- **Comprehensive Schema Support**: Handles real-world MCP tool schemas accurately
- **Flexible Configuration**: Multiple strategies for different CLI organization needs
- **Production Ready**: Full error handling, validation, and safety limits
- **Well Tested**: 25 passing tests with comprehensive coverage

This foundation is ready for integration with CLI generation systems and provides a clean, extensible architecture for future automated CLI updates while maintaining the existing manual CLI structure during transition.

## Implementation Complete

✅ **All implementation tasks completed successfully!**

### What Was Implemented

1. **CLI Generation Module Structure**: Created `swissarmyhammer-cli/src/generation/` with organized sub-modules:
   - `mod.rs` - Module entry point and re-exports
   - `types.rs` - Core data structures and configuration
   - `cli_generator.rs` - Main generation orchestrator 
   - `command_builder.rs` - Schema parsing and CLI structure building

2. **Core Components**:

   **CliGenerator**: Main orchestrator that:
   - Integrates with existing `ToolRegistry` CLI exclusion tracking
   - Respects CLI exclusion markers automatically
   - Supports configurable generation strategies
   - Enforces safety limits (max 1000 commands by default)
   - Handles error recovery gracefully

   **CommandBuilder**: Schema parser that:
   - Converts JSON Schema from MCP tools to CLI argument structures
   - Supports all common schema patterns (strings, integers, booleans, enums)
   - Handles validation constraints (minLength, pattern, minimum, etc.)
   - Intelligently categorizes parameters as arguments vs options
   - Applies naming transformations (underscores to dashes, etc.)

   **Data Structures**: Comprehensive type system including:
   - `GeneratedCommand` - Complete command representation
   - `CliArgument` - Required/optional command arguments with constraints
   - `CliOption` - Command-line flags and options with type information
   - `GenerationConfig` - Flexible configuration with multiple naming strategies

3. **Configuration System**: 
   - `NamingStrategy::KeepOriginal` - `issue_create` → `issue-create`
   - `NamingStrategy::GroupByDomain` - `issue_create` → `issue create` (subcommands)
   - `NamingStrategy::Flatten` - `issue_create` → `create-issue`
   - Support for command prefixes, subcommand organization, completion generation

4. **Integration with Existing Systems**:
   - ✅ Leverages existing CLI exclusion infrastructure from `ToolRegistry`
   - ✅ Respects `issue_work`, `issue_merge`, `abort_create` exclusions automatically
   - ✅ Maintains full backward compatibility with MCP operations
   - ✅ Added `async-trait` dependency to workspace configuration

5. **Comprehensive Testing**: All 31 tests pass (25 unit + 6 integration), covering:
   - CLI exclusion detection and respect
   - Schema parsing accuracy for various JSON Schema patterns  
   - Command structure generation and validation
   - Configuration options and naming strategies
   - Error handling and edge cases
   - Integration with tool registry systems

### Usage Example

```rust
use swissarmyhammer_cli::generation::{CliGenerator, GenerationConfig, NamingStrategy};
use swissarmyhammer_tools::ToolRegistry;
use std::sync::Arc;

// Create registry with tools (existing pattern)
let mut registry = ToolRegistry::new();
// Tools are automatically registered with CLI exclusion detection

// Generate CLI commands
let generator = CliGenerator::new(Arc::new(registry));
let commands = generator.generate_commands().unwrap();

// With custom configuration  
let config = GenerationConfig {
    naming_strategy: NamingStrategy::GroupByDomain,
    use_subcommands: true,
    command_prefix: Some("sah-".to_string()),
    ..Default::default()
};

let generator = generator.with_config(config);
let commands = generator.generate_commands().unwrap();
```

### Key Achievements

- **Zero Breaking Changes**: All existing code continues to work unchanged
- **Automatic CLI Exclusion**: Respects existing exclusion markers without manual configuration
- **Comprehensive Schema Support**: Handles real-world MCP tool schemas accurately
- **Flexible Configuration**: Multiple strategies for different CLI organization needs
- **Production Ready**: Full error handling, validation, and safety limits
- **Well Tested**: 31 passing tests with comprehensive coverage

This foundation is ready for integration with CLI generation systems and provides a clean, extensible architecture for future automated CLI updates while maintaining the existing manual CLI structure during transition.