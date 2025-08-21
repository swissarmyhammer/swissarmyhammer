# Shell Completion Generation and System Enhancement

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Implement shell completion generation for the dynamic CLI system and add final enhancements to complete the CLI architecture transformation.

## Implementation Tasks

### 1. Update Shell Completion Generation

Update `swissarmyhammer-cli/src/completions.rs` to work with dynamic CLI:

```rust
use clap_complete::{generate, Generator, Shell};
use std::io;
use anyhow::Result;

pub async fn handle_completion(shell: Shell) -> Result<()> {
    let cli = crate::build_dynamic_cli().await?;
    generate_completion(shell, &cli);
    Ok(())
}

fn generate_completion<G: Generator>(gen: G, cli: &clap::Command) {
    generate(gen, cli, cli.get_name(), &mut io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap_complete::shells::{Bash, Fish, Zsh};
    
    #[tokio::test]
    async fn test_bash_completion_generation() {
        let cli = crate::build_dynamic_cli().await.unwrap();
        
        // Test that completion generation doesn't panic
        let mut output = Vec::new();
        generate(Bash, &cli, "swissarmyhammer", &mut output);
        
        let completion_script = String::from_utf8(output).unwrap();
        
        // Verify completion script contains expected commands
        assert!(completion_script.contains("issue"));
        assert!(completion_script.contains("memo"));
        assert!(completion_script.contains("file"));
        assert!(completion_script.contains("create"));
        assert!(completion_script.contains("list"));
    }
    
    #[tokio::test]
    async fn test_completion_for_all_shells() {
        let cli = crate::build_dynamic_cli().await.unwrap();
        
        let shells = vec![
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
        ];
        
        for shell in shells {
            let mut output = Vec::new();
            generate(shell, &cli, "swissarmyhammer", &mut output);
            
            // Verify each shell generates non-empty completion
            assert!(!output.is_empty(), "Shell {:?} generated empty completion", shell);
        }
    }
}
```

### 2. Add CLI Performance Optimization

Create `swissarmyhammer-cli/src/cli_optimization.rs`:

```rust
use std::sync::OnceLock;
use std::collections::HashMap;
use clap::Command;
use anyhow::Result;

// Cache for CLI command structure to avoid rebuilding
static CLI_CACHE: OnceLock<Command> = OnceLock::new();
static TOOL_METADATA_CACHE: OnceLock<HashMap<String, crate::cli_builder::ToolMetadata>> = OnceLock::new();

/// Get cached CLI command or build it if not cached
pub async fn get_or_build_cli() -> Result<&'static Command> {
    if let Some(cli) = CLI_CACHE.get() {
        return Ok(cli);
    }
    
    let cli = crate::build_dynamic_cli().await?;
    CLI_CACHE.set(cli).map_err(|_| anyhow::anyhow!("Failed to cache CLI"))?;
    
    Ok(CLI_CACHE.get().unwrap())
}

/// Fast path for help commands that don't need full MCP initialization
pub fn is_help_command(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--help" || arg == "-h")
}

/// Fast path for version commands
pub fn is_version_command(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--version" || arg == "-V")
}

pub async fn handle_fast_path_commands(args: &[String]) -> Result<bool> {
    if is_help_command(args) || args.is_empty() {
        // For help, we can use a minimal CLI structure
        let cli = get_or_build_cli().await?;
        cli.clone().print_help()?;
        return Ok(true);
    }
    
    if is_version_command(args) {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(true);
    }
    
    Ok(false)
}
```

### 3. Add Advanced Schema Features Support

Enhance `swissarmyhammer-cli/src/schema_conversion.rs`:

```rust
impl SchemaConverter {
    /// Handle more advanced JSON Schema features
    pub fn enhanced_schema_to_clap_args(schema: &Value) -> Result<Vec<Arg>> {
        let mut args = Self::schema_to_clap_args(schema)?;
        
        // Post-process args for advanced features
        for arg in &mut args {
            Self::enhance_arg_with_schema_features(arg, schema)?;
        }
        
        Ok(args)
    }
    
    fn enhance_arg_with_schema_features(arg: &mut Arg, schema: &Value) -> Result<()> {
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            if let Some(prop_schema) = properties.get(arg.get_id().as_str()) {
                // Handle default values
                if let Some(default) = prop_schema.get("default") {
                    if let Some(default_str) = default.as_str() {
                        *arg = arg.clone().default_value(default_str);
                    }
                }
                
                // Handle examples in help text
                if let Some(examples) = prop_schema.get("examples").and_then(|e| e.as_array()) {
                    if !examples.is_empty() {
                        let example_text = examples.iter()
                            .filter_map(|e| e.as_str())
                            .take(2)
                            .collect::<Vec<_>>()
                            .join(", ");
                        
                        let current_help = arg.get_help().unwrap_or("");
                        let enhanced_help = format!("{}\n\nExamples: {}", current_help, example_text);
                        *arg = arg.clone().help(enhanced_help);
                    }
                }
                
                // Handle oneOf/anyOf for enum-like behavior
                if let Some(one_of) = prop_schema.get("oneOf").and_then(|o| o.as_array()) {
                    if let Some(enum_values) = Self::extract_enum_from_one_of(one_of) {
                        *arg = arg.clone().value_parser(enum_values);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn extract_enum_from_one_of(one_of: &[Value]) -> Option<Vec<String>> {
        let mut values = Vec::new();
        
        for item in one_of {
            if let Some(const_val) = item.get("const").and_then(|c| c.as_str()) {
                values.push(const_val.to_string());
            }
        }
        
        if values.is_empty() { None } else { Some(values) }
    }
}
```

### 4. Create CLI Migration Guide

Create `swissarmyhammer-cli/MIGRATION.md`:

```markdown
# CLI Migration Guide

## Overview

The CLI has been updated to use dynamic command generation from MCP tools, eliminating code duplication while maintaining full backward compatibility.

## For Users

### No Changes Required

All existing commands work exactly the same:

```bash
# These commands work identically to before
sah issue create "My Issue" --content "Issue description"
sah memo list
sah file read /path/to/file
sah search query "my search"
```

### New Features

- Help text is now automatically generated from MCP tool schemas
- New tools automatically appear in CLI without updates
- More consistent argument handling across all commands

## For Developers

### Architecture Changes

1. **Removed Redundant Enums**: The following enums have been removed (~425 lines):
   - `IssueCommands`
   - `MemoCommands` 
   - `FileCommands`
   - `SearchCommands`
   - `WebSearchCommands`
   - `ConfigCommands`
   - `ShellCommands`

2. **Dynamic Generation**: Commands are now generated at runtime from MCP tool registry

3. **Schema-Driven**: All CLI arguments are derived from JSON schemas

### Adding New CLI Commands

To add a new CLI command, simply:

1. Create an MCP tool implementing `McpTool` trait
2. Add CLI metadata methods:
   ```rust
   fn cli_category(&self) -> Option<&'static str> { Some("my_category") }
   fn cli_name(&self) -> &'static str { "my_command" }
   fn cli_about(&self) -> Option<&'static str> { Some("Command description") }
   ```
3. Register the tool in the registry

The command will automatically appear in CLI with proper help text and arguments.

### Testing Changes

- CLI integration tests now use the dynamic system
- Property-based tests validate schema conversion
- End-to-end tests ensure backward compatibility

## Troubleshooting

### Command Not Found

If a command is not found:

1. Check if the MCP tool is properly registered
2. Verify `cli_category()` and `cli_name()` are implemented
3. Ensure `hidden_from_cli()` returns false

### Help Text Issues

If help text is missing or incorrect:

1. Implement `cli_about()` for custom help text
2. Check JSON schema has proper `description` fields
3. Verify tool `description()` method returns useful text

### Argument Parsing Issues

If arguments don't parse correctly:

1. Check JSON schema property types match expected arguments
2. Verify required fields are marked in schema
3. Test schema conversion with unit tests
```

### 5. Add CLI Debug Mode

Create `swissarmyhammer-cli/src/debug.rs`:

```rust
use tracing::{info, debug, warn};
use std::env;

pub struct CliDebugger {
    enabled: bool,
}

impl CliDebugger {
    pub fn new() -> Self {
        Self {
            enabled: env::var("SAH_CLI_DEBUG").is_ok() || cfg!(debug_assertions),
        }
    }
    
    pub fn log_command_parsing(&self, args: &[String]) {
        if self.enabled {
            info!("Parsing CLI arguments: {:?}", args);
        }
    }
    
    pub fn log_dynamic_command_detection(&self, command_info: &crate::cli_builder::DynamicCommandInfo) {
        if self.enabled {
            info!("Detected dynamic command: {:?}", command_info);
        }
    }
    
    pub fn log_schema_conversion(&self, tool_name: &str, schema: &serde_json::Value) {
        if self.enabled {
            debug!("Converting schema for tool {}: {}", tool_name, 
                serde_json::to_string_pretty(schema).unwrap_or_else(|_| "Invalid JSON".to_string()));
        }
    }
    
    pub fn log_mcp_tool_execution(&self, tool_name: &str, arguments: &serde_json::Map<String, serde_json::Value>) {
        if self.enabled {
            info!("Executing MCP tool {} with arguments: {}", tool_name,
                serde_json::to_string_pretty(arguments).unwrap_or_else(|_| "Invalid JSON".to_string()));
        }
    }
}

/// Global debugger instance
pub static DEBUGGER: std::sync::LazyLock<CliDebugger> = std::sync::LazyLock::new(|| CliDebugger::new());
```

### 6. Final Integration and Documentation

Update `swissarmyhammer-cli/src/lib.rs`:

```rust
pub mod cli;
pub mod cli_builder;
pub mod schema_conversion;
pub mod dynamic_execution;
pub mod response_formatting;
pub mod cli_optimization;
pub mod debug;

// Static command modules (preserved)
pub mod serve;
pub mod doctor;
pub mod prompt;
pub mod flow;
pub mod completions;
pub mod validate;

// Utility modules
pub mod logging;
pub mod error;

pub use cli::{Cli, Commands, build_dynamic_cli};
pub use cli_builder::CliBuilder;
pub use dynamic_execution::DynamicCommandExecutor;
```

Update `README.md` section about the CLI:

```markdown
## Dynamic CLI Architecture

The CLI uses a dynamic command generation system that eliminates code duplication:

- **Single Source of Truth**: MCP tool schemas define both MCP and CLI interfaces
- **Automatic Updates**: New MCP tools automatically appear in CLI
- **Consistent Experience**: Schema-driven argument parsing and help generation
- **Backward Compatible**: All existing commands work identically

### Command Structure

- **Static Commands**: Core CLI functionality (serve, doctor, prompt, flow, etc.)
- **Dynamic Commands**: Generated from MCP tools (issue, memo, file, search, etc.)

The dynamic system generates over 30 commands and subcommands automatically from MCP tool definitions, eliminating ~425 lines of redundant CLI code.
```

## Success Criteria

- [ ] Shell completion generation works with dynamic commands
- [ ] CLI performance optimization reduces startup time
- [ ] Advanced schema features supported (defaults, examples, oneOf)
- [ ] Debug mode provides helpful troubleshooting information
- [ ] Migration guide documents all changes
- [ ] CLI caching improves help command performance
- [ ] Documentation updated to reflect new architecture
- [ ] All shell types generate proper completions
- [ ] Fast-path commands (help, version) work efficiently

## Architecture Notes

- Completes the transformation to dynamic CLI architecture
- Adds performance optimizations for better user experience
- Provides comprehensive documentation for developers
- Maintains full backward compatibility while enabling future extensibility
- Creates foundation for automatic CLI updates when MCP tools change

## Proposed Solution

After analyzing the existing completions.rs file and the dynamic CLI system, I will implement a comprehensive shell completion and enhancement system:

### Implementation Strategy:

#### 1. **Update Shell Completion Generation**
- Modify `completions.rs` to use the dynamic CLI builder instead of the static `Cli::command()`  
- Add async support for shell completion generation since dynamic CLI requires MCP tool registry initialization
- Create comprehensive tests to verify completions include all dynamic commands (issue, memo, file, search)
- Ensure completions work for all shell types (Bash, Zsh, Fish, PowerShell)

#### 2. **Add CLI Performance Optimization**
- Create `cli_optimization.rs` with CLI caching using `OnceLock` to avoid rebuilding CLI on every invocation
- Implement fast-path detection for help and version commands that don't need full MCP initialization  
- Add tool metadata caching to improve repeated CLI operations
- Optimize CLI startup time for common operations

#### 3. **Enhance Schema Conversion Features**
- Extend `schema_conversion.rs` with advanced JSON Schema features:
  - Default value support for CLI arguments
  - Example text integration in help messages
  - `oneOf`/`anyOf` enum-like behavior for argument validation
  - Better help text formatting and content

#### 4. **Add CLI Debug Mode**
- Create `debug.rs` with comprehensive debugging capabilities
- Environment variable-controlled debug output (`SAH_CLI_DEBUG`)
- Trace command parsing, dynamic command detection, schema conversion, and MCP tool execution
- Support both development and production debugging scenarios

#### 5. **Create Migration Documentation**
- Write comprehensive `MIGRATION.md` guide explaining the dynamic CLI changes
- Document backward compatibility guarantees for users
- Provide developer guide for adding new CLI commands via MCP tools
- Include troubleshooting section for common issues

#### 6. **Update Core Architecture Documentation**
- Update `lib.rs` to properly export new modules
- Enhance README.md with dynamic CLI architecture explanation
- Document the elimination of redundant CLI code (~425 lines removed)
- Explain the single-source-of-truth approach using MCP tool schemas

### Technical Approach:
- **Async Shell Completions**: Since dynamic CLI requires async initialization, I'll need to handle this properly in completion generation
- **Backward Compatibility**: All existing completion functionality must be preserved
- **Performance First**: Optimize for common CLI operations with caching and fast paths
- **Comprehensive Testing**: Each enhancement will include thorough testing to ensure reliability
- **Documentation Focus**: Clear migration guide and architecture documentation for developers

This solution completes the CLI architecture transformation while adding significant performance and usability improvements.