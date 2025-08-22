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