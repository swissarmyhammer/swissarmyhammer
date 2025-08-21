# CLI Migration Guide

## Overview

The SwissArmyHammer CLI has been transformed from a static command system to a dynamic command generation architecture. This migration eliminates code duplication while maintaining full backward compatibility and adding new capabilities.

## For Users

### 📢 No Changes Required

All existing commands work identically to before. Your scripts, aliases, and workflows will continue to function without modification:

```bash
# These commands work exactly the same as before
sah issue create "My Issue" --content "Issue description"
sah memo list
sah file read /path/to/file
sah search query "my search"
sah prompt list
sah flow run my-workflow
```

### 🆕 New Features

#### Enhanced Help Text
Help text is now automatically generated from MCP tool schemas, providing more accurate and detailed information:

```bash
$ sah issue create --help
# Now shows parameter types, examples, and better descriptions
```

#### Automatic Command Discovery
New MCP tools automatically appear in the CLI without requiring updates to the CLI codebase.

#### Consistent Argument Handling
All commands now use the same argument parsing and validation system, providing more consistent behavior across different command types.

#### Better Shell Completion
Shell completions now include all dynamic commands and their arguments, with more accurate suggestions.

## For Developers

### 🏗️ Architecture Changes

#### 1. Dynamic Command Generation
The CLI now builds its command structure at runtime from the MCP tool registry:

**Before (Static):**
```rust
enum IssueCommands {
    Create { name: String, content: String },
    List,
    Show { name: String },
    // ... many more variants
}
```

**After (Dynamic):**
```rust
// Commands are generated from MCP tool schemas
let cli = CliBuilder::new(tool_registry).build_cli()?;
```

#### 2. Eliminated Redundant Code
The following enums and structures have been removed (~425 lines):

- ❌ `IssueCommands` (17 variants)
- ❌ `MemoCommands` (7 variants)
- ❌ `FileCommands` (5 variants)
- ❌ `SearchCommands` (2 variants)
- ❌ `WebSearchCommands` (1 variant)
- ❌ `ConfigCommands` (4 variants)
- ❌ `ShellCommands` (1 variant)

#### 3. Schema-Driven Development
All CLI arguments are now derived from JSON schemas, ensuring consistency between MCP and CLI interfaces.

#### 4. Enhanced Error Handling
The dynamic system provides better error messages and validation, with debugging support through the `SAH_CLI_DEBUG` environment variable.

### 🔧 Adding New CLI Commands

Adding a new CLI command is now much simpler:

#### Step 1: Create MCP Tool
```rust
#[derive(Debug)]
pub struct MyNewTool;

impl McpTool for MyNewTool {
    fn name(&self) -> &str {
        "my_new_tool"
    }

    fn description(&self) -> &str {
        "Description of what this tool does"
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "Input parameter",
                    "examples": ["example1", "example2"]
                }
            },
            "required": ["input"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: Arc<ToolContext>) -> Result<ToolResult> {
        // Implementation
        Ok(ToolResult::new("Success"))
    }
}
```

#### Step 2: Add CLI Metadata
```rust
impl McpTool for MyNewTool {
    // ... existing implementation

    fn cli_category(&self) -> Option<&'static str> {
        Some("mytools")  // Creates `sah mytools` category
    }

    fn cli_name(&self) -> &'static str {
        "new-command"   // Creates `sah mytools new-command`
    }

    fn cli_about(&self) -> Option<&'static str> {
        Some("Custom help text for this command")
    }

    fn hidden_from_cli(&self) -> bool {
        false  // Make visible in CLI
    }
}
```

#### Step 3: Register Tool
```rust
pub fn register_my_tools(registry: &mut ToolRegistry) {
    registry.register_tool(MyNewTool);
}
```

The command will automatically appear in the CLI with:
- Proper help text from schema descriptions
- Argument validation from schema types
- Shell completion support
- Consistent error handling

### 🧪 Testing Changes

#### CLI Integration Tests
The dynamic system is thoroughly tested with property-based tests, integration tests, and backward compatibility tests:

```rust
#[tokio::test]
async fn test_dynamic_command_generation() {
    let cli = build_dynamic_cli().await.unwrap();
    
    // Verify all expected commands are present
    assert!(cli.find_subcommand("issue").is_some());
    assert!(cli.find_subcommand("memo").is_some());
    
    // Test help generation works
    let mut help_output = Vec::new();
    cli.write_help(&mut help_output).unwrap();
    assert!(!help_output.is_empty());
}
```

#### Property-Based Testing
Schema conversion is validated with fuzzing:

```rust
proptest! {
    #[test]
    fn test_schema_conversion_robust(
        prop_name in "[a-zA-Z][a-zA-Z0-9_]*",
        description in ".*",
    ) {
        let schema = json!({
            "type": "object",
            "properties": {
                prop_name: {"type": "string", "description": description}
            }
        });
        
        let result = SchemaConverter::schema_to_clap_args(&schema);
        prop_assert!(result.is_ok());
    }
}
```

## 🔧 Migration Troubleshooting

### Command Not Found

If a command that used to work is not found:

1. **Check MCP Tool Registration**: Ensure the tool is properly registered in the tool registry
2. **Verify CLI Metadata**: Check that `cli_category()` and `cli_name()` are implemented correctly
3. **Check Hidden Status**: Ensure `hidden_from_cli()` returns `false`

```rust
// Example debugging
fn cli_category(&self) -> Option<&'static str> {
    Some("issue")  // Must match existing or desired category
}

fn cli_name(&self) -> &'static str {
    "create"  // Must be unique within category
}

fn hidden_from_cli(&self) -> bool {
    false  // Must be false to appear in CLI
}
```

### Help Text Issues

If help text is missing or incorrect:

1. **Implement Custom Help**: Use `cli_about()` for tool-specific help
2. **Check Schema Descriptions**: Verify JSON schema has proper `description` fields
3. **Verify Tool Description**: Check that `description()` method returns useful text

```rust
fn cli_about(&self) -> Option<&'static str> {
    Some("Create a new issue with title and content")
}

fn input_schema(&self) -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "description": "Issue title (will appear in help text)"
            }
        }
    })
}
```

### Argument Parsing Issues

If arguments don't parse correctly:

1. **Check Schema Types**: Verify JSON schema property types match expected CLI arguments
2. **Verify Required Fields**: Ensure required fields are marked correctly in schema
3. **Test Schema Conversion**: Use unit tests to validate schema-to-args conversion

```rust
// Correct schema for CLI argument
{
    "name": {
        "type": "string",        // Correct: will create --name flag
        "description": "Name"    // Will appear in help
    },
    "count": {
        "type": "integer",       // Correct: will validate as number
        "minimum": 1             // Will show in help text
    }
}

// In required array for positional args
"required": ["name"]  // Makes "name" a positional argument
```

### Performance Issues

If CLI startup is slow:

1. **Check Debug Mode**: Disable `SAH_CLI_DEBUG` in production
2. **MCP Timeout**: Adjust `SAH_MCP_TIMEOUT` environment variable
3. **Cache Usage**: The CLI caches built commands automatically

```bash
# Disable debug logging for better performance
unset SAH_CLI_DEBUG

# Increase MCP timeout if needed (default: 10s normal, 300s CI)
export SAH_MCP_TIMEOUT=30

# Check cache status
SAH_CLI_DEBUG=1 sah --help 2>&1 | grep cache
```

## 🚀 Performance Improvements

### CLI Caching
The dynamic CLI system includes intelligent caching:

```rust
// CLI structure is cached after first build
static CLI_CACHE: OnceLock<Command> = OnceLock::new();

// Fast path for common operations
if is_help_command(args) {
    // Uses cached CLI or builds minimal version
}
```

### Fast Path Commands
Common commands like `--help` and `--version` use optimized execution paths:

```bash
sah --help      # Fast path - no full MCP initialization
sah --version   # Fast path - immediate response
sah issue --help # Dynamic path - uses full CLI structure
```

### Reduced Memory Usage
Dynamic generation eliminates:
- 425+ lines of enum definitions
- Duplicate argument parsing logic  
- Redundant help text definitions
- Multiple CLI command trees

## 🔍 Debugging

### Enable Debug Mode
```bash
# Basic debug information
export SAH_CLI_DEBUG=1
sah issue create test-issue

# Verbose debug information  
export SAH_CLI_DEBUG_VERBOSE=1
sah issue create test-issue
```

### Debug Output Examples
```bash
🔍 Parsing CLI arguments: ["sah", "issue", "create", "test"]
🏗️  Building dynamic CLI with 25 tools in 4 categories
✅ Detected dynamic command: DynamicCommandInfo { category: Some("issue"), tool_name: "create", mcp_tool_name: "issue_create" }
🔧 Converting schema for tool: issue_create
🚀 Executing MCP tool: issue_create
```

## 📊 Migration Metrics

### Code Reduction
- **Removed Lines**: ~425 lines of enum definitions
- **Removed Files**: 0 (refactored existing files)
- **Added Lines**: ~800 lines of dynamic infrastructure
- **Net Effect**: +375 lines, but eliminates future duplication

### Functionality Gain
- **Auto-discovery**: New MCP tools automatically appear in CLI
- **Consistency**: Single source of truth for arguments and help
- **Extensibility**: Easy to add new commands without CLI changes
- **Maintainability**: Schema changes automatically update CLI

### Performance Impact
- **Startup Time**: +~200ms for dynamic CLI building (cached after first run)
- **Memory Usage**: -~50KB from eliminated enum definitions
- **Help Generation**: Faster due to caching
- **Shell Completion**: More comprehensive and accurate

## 🎯 Future Roadmap

### Phase 1: Stability (Current)
- ✅ Core dynamic CLI infrastructure
- ✅ Backward compatibility maintenance
- ✅ Comprehensive testing suite
- ✅ Performance optimizations

### Phase 2: Enhancement (Next)
- 🔄 Advanced schema features (conditionals, dependencies)
- 🔄 Interactive command wizards
- 🔄 Command aliases and shortcuts
- 🔄 Plugin system for external tools

### Phase 3: Integration (Future)
- 📋 IDE integration improvements
- 📋 Advanced completion with context awareness  
- 📋 Command history and favorites
- 📋 Performance analytics and optimization

## 🤝 Contributing

### Adding New Tools
1. Implement the `McpTool` trait
2. Add CLI metadata methods
3. Register in appropriate module
4. Add integration tests
5. Update documentation

### Testing Changes
```bash
# Run all CLI tests
cargo test --package swissarmyhammer-cli

# Run specific test suites
cargo test test_dynamic_cli
cargo test test_completion
cargo test test_schema_conversion

# Run property-based tests
cargo test proptest

# Run integration tests
cargo test --test integration
```

### Debugging Tools
```bash
# Schema validation
cargo run --bin validate-schemas

# CLI structure inspection  
SAH_CLI_DEBUG=1 cargo run -- --help

# Performance profiling
cargo run --release --features profiling -- issue list
```

## 📞 Support

If you encounter issues with the migration:

1. **Check Environment**: Ensure no conflicting environment variables are set
2. **Update Dependencies**: Run `cargo update` to get latest versions
3. **Clear Cache**: Remove `.swissarmyhammer/` directory to clear state
4. **Enable Debugging**: Use `SAH_CLI_DEBUG=1` to see detailed execution
5. **Report Issues**: Use GitHub issues for reproducible problems

The dynamic CLI system is designed to be fully backward compatible. If you encounter any breaking changes, please report them as bugs.