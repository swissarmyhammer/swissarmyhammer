# Integrate Dynamic CLI Builder with Main CLI

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective
Integrate the `CliBuilder` with the main CLI entry point, enabling dynamic command generation while preserving static commands and existing functionality.

## Technical Details

### Main CLI Integration
Update `swissarmyhammer-cli/src/main.rs`:

```rust
use swissarmyhammer_cli::dynamic_cli::CliBuilder;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tool registry
    let tool_registry = Arc::new(ToolRegistry::default());
    
    // Build CLI with both static and dynamic commands
    let cli_builder = CliBuilder::new(tool_registry.clone());
    let cli = cli_builder.build_cli();
    
    // Parse arguments
    let matches = cli.try_get_matches()?;
    
    // Handle command dispatch
    dispatch_command(matches, tool_registry).await?;
    
    Ok(())
}
```

### Command Dispatch Logic
Update command dispatch to handle both static and dynamic commands:

```rust
async fn dispatch_command(
    matches: ArgMatches,
    tool_registry: Arc<ToolRegistry>
) -> Result<(), Box<dyn std::error::Error>> {
    match matches.subcommand() {
        // Static commands (unchanged)
        Some(("serve", _)) => handle_serve_command().await?,
        Some(("doctor", sub_matches)) => handle_doctor_command(sub_matches).await?,
        Some(("prompt", sub_matches)) => handle_prompt_command(sub_matches).await?,
        // ... other static commands
        
        // Dynamic MCP tool commands
        Some((category, sub_matches)) => {
            if let Some((tool_name, tool_matches)) = sub_matches.subcommand() {
                handle_dynamic_command(category, tool_name, tool_matches, tool_registry, context).await?;
            }
        }
        
        None => {
            // Print help and exit
            cli.print_help()?;
        }
    }
    
    Ok(())
}
```

### Feature Flag Support  
Add feature flag to control dynamic CLI generation during migration:

```rust
#[cfg(feature = "dynamic-cli")]
fn build_cli_with_dynamic(tool_registry: Arc<ToolRegistry>) -> Command {
    CliBuilder::new(tool_registry).build_cli()
}

#[cfg(not(feature = "dynamic-cli"))]  
fn build_cli_static() -> Command {
    // Existing static CLI builder
    Cli::command()
}
```

### Cargo.toml Feature
Add to `swissarmyhammer-cli/Cargo.toml`:
```toml
[features]
default = []
dynamic-cli = []
```

### Error Handling Integration
Ensure dynamic command errors integrate with existing CLI error handling:

```rust
match handle_dynamic_command(...).await {
    Ok(()) => {},
    Err(e) => {
        eprintln!("Error: {}", e);
        std::process::exit(EXIT_ERROR);
    }
}
```

### Context Initialization
Handle MCP context initialization for dynamic commands:

```rust
async fn create_tool_context() -> Result<Arc<ToolContext>, Box<dyn std::error::Error>> {
    // Initialize tool context similar to existing MCP integration
    let context = CliToolContext::new().await?;
    Ok(context.into_tool_context())
}
```

## Acceptance Criteria
- [ ] Dynamic CLI builder integrated with main CLI
- [ ] Both static and dynamic commands work correctly
- [ ] Feature flag support for gradual rollout
- [ ] Existing static commands unchanged and functional
- [ ] Dynamic command dispatch to MCP tools
- [ ] Error handling integration
- [ ] Context initialization for MCP tools
- [ ] Help generation includes both static and dynamic commands
- [ ] CLI startup performance not significantly impacted

## Implementation Notes
- Use feature flag to safely test integration
- Ensure backward compatibility during transition
- Handle edge cases like unknown commands gracefully
- Consider CLI startup time with tool registry initialization
- Plan for rollback if issues arise during integration

## Proposed Solution

After analyzing the existing codebase and the CLI architecture specification in `/ideas/cli.md`, I propose the following implementation approach:

### 1. Create Dynamic CLI Builder Module

Create `swissarmyhammer-cli/src/dynamic_cli.rs` implementing the `CliBuilder` as described in the specification. The builder will:
- Generate dynamic commands from `ToolRegistry` using the existing `cli_category()` and `cli_name()` methods on `McpTool` trait
- Convert JSON schemas to clap arguments using `schema_to_clap_args()`
- Build category-based subcommands (e.g., `sah memo create`, `sah file read`)

### 2. Feature Flag Integration

Add feature flag support in `Cargo.toml`:
```toml
[features]
default = []
dynamic-cli = []
```

### 3. Main CLI Integration

Update `main.rs` to:
- Initialize `ToolRegistry` with all MCP tools
- Use feature flag to conditionally build dynamic CLI alongside static commands
- Add new command dispatch logic for dynamic commands while preserving existing static commands

### 4. Command Dispatch Enhancement

Extend command dispatch in `main.rs` to handle:
- Existing static commands (unchanged)
- New dynamic MCP tool commands via category/tool name routing
- Context initialization for MCP tools using existing `ToolContext`

### 5. Gradual Migration Path

- Start with feature flag disabled by default 
- Existing CLI functionality remains unchanged
- Dynamic commands available when feature is enabled
- Allows testing and validation before full rollout

### Implementation Steps

1. Create `dynamic_cli.rs` module with `CliBuilder` struct
2. Add feature flag to `Cargo.toml`
3. Update `main.rs` with conditional CLI building
4. Add dynamic command dispatch logic
5. Test both static and dynamic command modes
6. Update documentation and help generation

This approach follows the established pattern in the codebase and provides a safe migration path while preserving all existing functionality.

## Implementation Results

### ✅ Completed Features

1. **Feature Flag Support** - Added `dynamic-cli` feature to `swissarmyhammer-cli/Cargo.toml`
2. **CLI Builder Module** - Created `swissarmyhammer-cli/src/dynamic_cli.rs` with `CliBuilder` struct
3. **Main CLI Integration** - Updated `main.rs` with conditional compilation and separate execution paths
4. **MCP Context Integration** - Enhanced `CliToolContext` with `get_tool_registry()` method
5. **Command Dispatch Logic** - Implemented dynamic command handling functions
6. **Backward Compatibility** - Existing static CLI functionality preserved and tested

### 🔄 Current Status

- **Static CLI Mode**: ✅ Fully functional - all existing commands work as before
- **Dynamic CLI Mode**: ⚠️ Blocked by Rust lifetime constraints with clap library

### 🚧 Technical Challenge: Clap Lifetime Issues

The dynamic CLI implementation encounters fundamental lifetime issues with the clap library:

```rust
error[E0521]: borrowed data escapes outside of method
  --> swissarmyhammer-cli/src/dynamic_cli.rs:63:23
   |                       `category` escapes the method body here
   |                       argument requires that `'1` must outlive `'static`
```

**Root Cause**: Clap requires `'static` lifetimes for command names, descriptions, and arguments, but our dynamic approach needs to borrow data from MCP tool schemas at runtime.

**Attempted Solutions**:
1. ✗ String ownership with `.to_string()` - still has lifetime issues
2. ✗ Reference-based approach - fails clap's `'static` requirements  
3. ✗ Various clap type conversions - incompatible trait bounds

### 📋 Next Steps & Alternative Approaches

**Option 1: Pre-compute Static Data**
- Generate static CLI definitions at compile-time from MCP schemas
- Use build.rs to create static command definitions
- Requires tooling changes but avoids runtime lifetime issues

**Option 2: Simplified Dynamic Integration**
- Keep existing CLI commands as-is
- Add only essential dynamic commands (e.g., generic MCP tool runner)
- Reduce scope to avoid complex schema-to-clap conversion

**Option 3: Different CLI Library**
- Evaluate alternative CLI libraries with more flexible lifetime requirements
- Consider structopt/clap alternatives that work better with runtime data

### 🎯 Current Recommendation

For immediate deployment, the feature flag approach allows:
- **Production use**: Static CLI mode (default) - fully functional
- **Development**: Dynamic CLI exploration when feature is enabled
- **Gradual migration**: Can implement Option 1 or 2 incrementally

The integration infrastructure is complete and ready for any of the alternative approaches above.