# Migrate Serve Command to CliContext Pattern and Consistent Output

## Overview

Migrate the `sah serve` command to use CliContext pattern and fix multiple architectural issues including scattered output, manual library creation, and inconsistent error handling.

## Current Serve Command Issues

### 1. Uses TemplateContext Instead of CliContext
**Current signature**:
```rust
pub async fn handle_command(
    matches: &clap::ArgMatches,
    _template_context: &TemplateContext,
) -> i32
```

**Problems**:
- No support for global `--verbose` and `--format` arguments
- Inconsistent with migrated commands (doctor, implement, flow)
- Cannot use CliContext display methods or cached resources

### 2. Scattered Output with Mixed Formatting
**Current HTTP serve output**:
```
Starting SwissArmyHammer MCP server on 127.0.0.1:8000    # println!
✅ MCP HTTP server running on http://127.0.0.1:8000       # println!
💡 Use Ctrl+C to stop the server                         # println!
🔍 Health check: http://127.0.0.1:8000/health            # println!
🛑 Shutting down server...                               # println!
✅ Server stopped                                         # println!
```

**Problems**:
- Multiple direct println! calls instead of structured output
- No support for JSON/YAML output formats
- Manual formatting instead of using display abstractions

### 3. Recreates Expensive Objects
**Current stdio serve logic**:
```rust
// Create library and load prompts
let mut library = PromptLibrary::new();
let mut resolver = PromptResolver::new();
if let Err(e) = resolver.load_all_prompts(&mut library) {
    // ...
}
```

**Problems**:
- Recreates PromptLibrary instead of using cached version from CliContext
- Duplicates prompt loading logic that's already done in CliContext
- Inefficient resource usage

## Solution: Apply Established Patterns

### 1. Update to CliContext Integration

**Target signature**:
```rust
pub async fn handle_command(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
) -> i32
```

### 2. Create Display Objects for Server Status

**File**: `swissarmyhammer-cli/src/commands/serve/display.rs`

```rust
use tabled::Tabled;
use serde::Serialize;

#[derive(Tabled, Serialize)]
pub struct ServerStatus {
    #[tabled(rename = "Status")]
    pub status: String,
    
    #[tabled(rename = "Action")]
    pub action: String,
    
    #[tabled(rename = "Details")]
    pub details: String,
}

#[derive(Tabled, Serialize)]
pub struct VerboseServerStatus {
    #[tabled(rename = "Status")]
    pub status: String,
    
    #[tabled(rename = "Action")]
    pub action: String,
    
    #[tabled(rename = "Details")]
    pub details: String,
    
    #[tabled(rename = "Endpoint")]
    pub endpoint: String,
    
    #[tabled(rename = "Info")]
    pub info: String,
}
```

### 3. Use CliContext Resources

**Updated stdio serve logic**:
```rust
async fn handle_stdio_serve(cli_context: &CliContext) -> i32 {
    if cli_context.verbose {
        println!("Starting MCP server in stdio mode");
    }

    // Use prompt library from CliContext instead of recreating
    let library = cli_context.get_prompt_library();

    if cli_context.verbose {
        let prompt_count = library.list().map(|p| p.len()).unwrap_or(0);
        println!("Using {} prompts for MCP server", prompt_count);
    }

    // Rest of server setup using existing resources
    // ...
}
```

### 4. Structured Output Using CliContext

**Replace scattered println! with structured display**:

**HTTP serve status**:
```rust
async fn handle_http_serve(matches: &clap::ArgMatches, cli_context: &CliContext) -> i32 {
    let port: u16 = matches.get_one::<u16>("port").copied().unwrap_or(8000);
    let host = matches.get_one::<String>("host").map(|s| s.as_str()).unwrap_or("127.0.0.1");
    let bind_addr = format!("{}:{}", host, port);

    // Collect server status for display
    let mut status_updates = Vec::new();

    status_updates.push(ServerStatus {
        status: "🚀".to_string(),
        action: "Starting Server".to_string(),
        details: format!("Binding to {}", bind_addr),
    });

    match start_http_server(&bind_addr).await {
        Ok(handle) => {
            status_updates.push(ServerStatus {
                status: "✅".to_string(),
                action: "Server Running".to_string(),
                details: format!("Available at {}", handle.url()),
            });

            if cli_context.verbose {
                status_updates.push(ServerStatus {
                    status: "💡".to_string(),
                    action: "Control".to_string(),
                    details: "Use Ctrl+C to stop".to_string(),
                });

                status_updates.push(ServerStatus {
                    status: "🔍".to_string(),
                    action: "Health Check".to_string(),
                    details: format!("{}/health", handle.url()),
                });
            }

            // Display status using CliContext
            cli_context.display(status_updates)?;

            // Wait for shutdown
            wait_for_shutdown().await;

            // Shutdown status
            let shutdown_status = vec![ServerStatus {
                status: "🛑".to_string(),
                action: "Shutting Down".to_string(),
                details: "Server stopping...".to_string(),
            }];
            cli_context.display(shutdown_status)?;

            // Handle shutdown result
            match handle.shutdown().await {
                Ok(_) => {
                    let success_status = vec![ServerStatus {
                        status: "✅".to_string(),
                        action: "Server Stopped".to_string(),
                        details: "Shutdown complete".to_string(),
                    }];
                    cli_context.display(success_status)?;
                    EXIT_SUCCESS
                }
                Err(e) => {
                    let error_status = vec![ServerStatus {
                        status: "⚠️".to_string(),
                        action: "Shutdown Warning".to_string(),
                        details: format!("Server shutdown error: {}", e),
                    }];
                    cli_context.display(error_status)?;
                    EXIT_WARNING
                }
            }
        }
        Err(e) => {
            let error_status = vec![ServerStatus {
                status: "❌".to_string(),
                action: "Server Failed".to_string(),
                details: format!("Failed to start: {}", e),
            }];
            cli_context.display(error_status)?;
            EXIT_ERROR
        }
    }
}
```

## Implementation Steps

### 1. Update Command Signature

**File**: `swissarmyhammer-cli/src/commands/serve/mod.rs`

**Change from**:
```rust
pub async fn handle_command(
    matches: &clap::ArgMatches,
    _template_context: &TemplateContext,
) -> i32
```

**Change to**:
```rust
pub async fn handle_command(
    matches: &clap::ArgMatches,
    cli_context: &CliContext,
) -> i32
```

### 2. Create Display Objects

**File**: `swissarmyhammer-cli/src/commands/serve/display.rs`

Create structured display objects for server status updates and lifecycle events.

### 3. Use CliContext Resources

**Remove library recreation**:
- Use `cli_context.get_prompt_library()` instead of creating new library
- Remove manual prompt loading in stdio serve
- Leverage cached resources from CliContext

### 4. Apply Consistent Output Formatting

**Replace scattered println! with**:
- Structured status objects
- CliContext display methods  
- Support for global `--verbose` and `--format` arguments

### 5. Update Main.rs Integration

**File**: `swissarmyhammer-cli/src/main.rs`

**Change from**:
```rust
commands::serve::handle_command(sub_matches, template_context).await
```

**Change to**:
```rust
commands::serve::handle_command(sub_matches, &cli_context).await
```

## Expected Benefits

### For Users
- **Global arguments work**: `sah --verbose serve`, `sah --format=json serve http`
- **Consistent output**: Same formatting as other commands
- **Better status tracking**: Clear server lifecycle status
- **Scriptable output**: JSON output for automation

### For Architecture
- **Pattern completion**: All major commands use CliContext
- **Resource efficiency**: Reuse cached prompt library
- **Consistent error handling**: Same patterns across commands
- **Reduced duplication**: No manual library recreation

## Success Criteria

1. ✅ `sah serve` works exactly as before (stdio mode)
2. ✅ `sah serve http` works exactly as before (HTTP mode)
3. ✅ `sah --verbose serve` shows detailed server status
4. ✅ `sah --format=json serve` outputs structured JSON status
5. ✅ Uses CliContext prompt library instead of recreating
6. ✅ Consistent error handling and exit codes
7. ✅ All server status output through structured display objects

## Files Created

- `swissarmyhammer-cli/src/commands/serve/display.rs` - Server status display objects

## Files Modified

- `swissarmyhammer-cli/src/commands/serve/mod.rs` - CliContext integration, structured output
- `swissarmyhammer-cli/src/main.rs` - Pass CliContext instead of TemplateContext

---

**Priority**: Medium - Completes CliContext pattern across all static commands
**Estimated Effort**: Medium (server status formatting + CliContext integration)
**Dependencies**: cli_prompt_000001_add_global_format_argument
**Benefits**: Consistency, resource efficiency, global argument support