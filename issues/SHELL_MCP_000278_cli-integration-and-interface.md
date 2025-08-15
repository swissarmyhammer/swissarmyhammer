# CLI Integration and Interface Implementation

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Implement CLI subcommand integration for the shell MCP tool, providing direct command-line access while following established CLI patterns and user experience guidelines.

## Objective

Create a comprehensive CLI interface for the shell tool that integrates seamlessly with the existing SwissArmyHammer CLI architecture and provides intuitive shell command execution.

## Requirements

### CLI Command Structure
- Add `shell` subcommand to main CLI
- Support all shell tool parameters via CLI flags
- Provide interactive and batch execution modes
- Follow existing CLI patterns and conventions

### Parameter Mapping
- Map CLI arguments to MCP tool parameters
- Support working directory specification
- Enable timeout configuration
- Allow environment variable passing

### Output Formatting
- Support multiple output formats (table, json, yaml)
- Provide human-readable default formatting
- Handle command output display appropriately
- Include execution metadata when requested

### User Experience
- Comprehensive help and documentation
- Clear error messages and suggestions
- Progress indicators for long-running commands
- Intuitive parameter naming and organization

## Implementation Details

### CLI Command Structure
```rust
#[derive(Debug, Parser)]
#[command(name = "shell", about = "Execute shell commands with timeout and output capture")]
pub struct ShellCommand {
    /// Shell command to execute
    #[arg(value_name = "COMMAND")]
    pub command: String,
    
    /// Working directory for command execution
    #[arg(short = 'C', long = "directory", value_name = "DIR")]
    pub working_directory: Option<PathBuf>,
    
    /// Command timeout in seconds (default: 300, max: 1800)
    #[arg(short = 't', long = "timeout", value_name = "SECONDS", default_value = "300")]
    pub timeout: u64,
    
    /// Set environment variables (KEY=VALUE format)
    #[arg(short = 'e', long = "env", value_name = "KEY=VALUE")]
    pub environment: Vec<String>,
    
    /// Output format
    #[arg(long, value_enum, default_value = "human")]
    pub format: OutputFormat,
    
    /// Show execution metadata
    #[arg(long)]
    pub show_metadata: bool,
    
    /// Quiet mode - suppress command output, show only results
    #[arg(short = 'q', long)]
    pub quiet: bool,
}
```

### MCP Integration Handler
```rust
use crate::mcp_integration::CliToolContext;

pub async fn handle_shell_command(
    command: ShellCommand
) -> Result<(), Box<dyn std::error::Error>> {
    let context = CliToolContext::new().await?;
    
    // Parse environment variables
    let env_vars = parse_environment_variables(&command.environment)?;
    
    // Build MCP tool arguments
    let args = context.create_arguments(vec![
        ("command", json!(command.command)),
        ("working_directory", json!(command.working_directory.map(|d| d.display().to_string()))),
        ("timeout", json!(command.timeout)),
        ("environment", json!(env_vars)),
    ]);
    
    // Execute shell command via MCP
    let result = context.execute_tool("shell_execute", args).await?;
    
    // Format and display results
    display_shell_results(result, &command).await?;
    
    Ok(())
}
```

### Environment Variable Parsing
```rust
fn parse_environment_variables(
    env_args: &[String]
) -> Result<Option<HashMap<String, String>>, CliError> {
    if env_args.is_empty() {
        return Ok(None);
    }
    
    let mut env_vars = HashMap::new();
    
    for env_arg in env_args {
        if let Some((key, value)) = env_arg.split_once('=') {
            env_vars.insert(key.to_string(), value.to_string());
        } else {
            return Err(CliError::InvalidArgument {
                argument: "environment".to_string(),
                value: env_arg.clone(),
                expected: "KEY=VALUE format".to_string(),
            });
        }
    }
    
    Ok(Some(env_vars))
}
```

### Output Formatting and Display
```rust
async fn display_shell_results(
    result: Value,
    command: &ShellCommand
) -> Result<(), CliError> {
    let metadata = result["metadata"].as_object()
        .ok_or_else(|| CliError::InvalidResponse("Missing metadata".to_string()))?;
    
    match command.format {
        OutputFormat::Human => display_human_format(result, command),
        OutputFormat::Json => display_json_format(result),
        OutputFormat::Yaml => display_yaml_format(result),
    }
}

fn display_human_format(result: Value, command: &ShellCommand) -> Result<(), CliError> {
    let metadata = result["metadata"].as_object().unwrap();
    let exit_code = metadata["exit_code"].as_i64().unwrap_or(0);
    let stdout = metadata["stdout"].as_str().unwrap_or("");
    let stderr = metadata["stderr"].as_str().unwrap_or("");
    
    // Display command output
    if !command.quiet && !stdout.is_empty() {
        println!("{}", stdout);
    }
    
    if !stderr.is_empty() {
        eprintln!("{}", stderr);
    }
    
    // Display metadata if requested
    if command.show_metadata {
        display_execution_metadata(metadata)?;
    }
    
    // Set appropriate exit code
    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
    
    Ok(())
}
```

### Help and Documentation
```rust
impl ShellCommand {
    pub fn long_about() -> &'static str {
        r#"Execute shell commands with comprehensive timeout controls and output capture.

EXAMPLES:
    # Basic command execution
    sah shell "ls -la"
    
    # Execute in specific directory
    sah shell -C /project "cargo build"
    
    # Set timeout and environment variables
    sah shell -t 600 -e "RUST_LOG=debug" -e "BUILD_ENV=production" "./build.sh"
    
    # Show execution metadata
    sah shell --show-metadata "uname -a"
    
    # Quiet mode with JSON output
    sah shell --quiet --format json "git status --porcelain"

SECURITY:
    Commands are validated for basic safety patterns. Dangerous commands
    like 'rm -rf /' are blocked by default. Directory access may be restricted
    based on configuration.
    
TIMEOUTS:
    Default timeout is 5 minutes (300 seconds). Maximum timeout is 30 minutes
    (1800 seconds). Commands are terminated cleanly on timeout."#
    }
}
```

## Integration Points

### Main CLI Integration
- Add shell command to main CLI dispatcher
- Follow existing command organization patterns
- Integrate with global CLI flags and options
- Maintain consistency with other subcommands

### MCP Tool Context
- Use existing `CliToolContext` for MCP communication
- Follow established patterns from other CLI commands
- Handle MCP errors and convert to CLI errors appropriately
- Maintain consistent error handling

### Output and Logging
- Integrate with existing output formatting systems
- Use established logging patterns
- Handle terminal detection and color output
- Support different verbosity levels

## Acceptance Criteria

- [ ] Shell subcommand integrated into main CLI
- [ ] All shell tool parameters accessible via CLI flags
- [ ] Environment variable parsing works correctly
- [ ] Output formatting supports multiple formats
- [ ] Help documentation comprehensive and clear
- [ ] Error handling provides helpful messages
- [ ] Exit codes reflect command execution results
- [ ] Integration with existing CLI patterns maintained

## Testing Requirements

- [ ] CLI argument parsing tests
- [ ] Environment variable parsing tests
- [ ] Output formatting tests
- [ ] MCP integration tests via CLI
- [ ] Error handling and exit code tests
- [ ] Help text and documentation tests

## Usage Examples

### Basic Commands
```bash
# Simple command execution
sah shell "echo 'Hello, World!'"

# Directory listing with metadata
sah shell --show-metadata "ls -la /tmp"
```

### Development Workflows
```bash
# Build with timeout and environment
sah shell -C /project -t 600 -e "CARGO_FEATURES=release" "cargo build"

# Test execution with custom environment
sah shell -e "TEST_ENV=integration" -e "RUST_LOG=debug" "cargo test"
```

### System Operations
```bash
# System information gathering
sah shell --format json "uname -a && df -h"

# Process monitoring
sah shell -t 60 "top -b -n 1"
```

## Notes

- CLI integration provides direct access without requiring MCP server setup
- Focus on developer-friendly command execution workflows
- Maintain security controls even in CLI mode
- Support both interactive and scripting use cases
- Ensure exit codes properly reflect command execution status