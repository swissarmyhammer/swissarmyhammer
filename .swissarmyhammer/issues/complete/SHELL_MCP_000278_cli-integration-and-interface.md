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
## Proposed Solution

Based on my analysis of the existing codebase, I will implement the CLI integration by following the established patterns:

### Implementation Steps

1. **Register Shell Tools in MCP Integration** (`swissarmyhammer-cli/src/mcp_integration.rs`):
   - Add shell tool registration to the `create_tool_registry()` function
   - Import `register_shell_tools` from swissarmyhammer-tools

2. **Add Shell Command to CLI Definition** (`swissarmyhammer-cli/src/cli.rs`):
   - Add `Shell` variant to `Commands` enum
   - Create `ShellCommands` enum with `Execute` subcommand
   - Define comprehensive CLI parameters matching the MCP tool schema:
     - `command` (required): Shell command string
     - `working_directory` (-C, --directory): Optional working directory
     - `timeout` (-t, --timeout): Timeout in seconds (default 300, max 1800)
     - `environment` (-e, --env): Environment variables as KEY=VALUE pairs
     - `format` (--format): Output format (human, json, yaml)
     - `show_metadata` (--show-metadata): Include execution metadata
     - `quiet` (-q, --quiet): Suppress command output

3. **Create Shell CLI Handler Module** (`swissarmyhammer-cli/src/shell.rs`):
   - Implement `handle_shell_command()` function following existing patterns
   - Parse environment variables from CLI arguments
   - Build MCP tool arguments using `CliToolContext`
   - Execute shell command via `shell_execute` MCP tool
   - Format and display results with appropriate output handling
   - Handle timeouts and errors gracefully
   - Provide exit codes that reflect command execution status

4. **Add Shell Command Registration** (`swissarmyhammer-cli/src/main.rs`):
   - Add shell module import
   - Add shell command handling to main dispatcher
   - Use existing `run_shell` async function pattern

5. **Implement Output Formatting**:
   - Human-readable format: Display stdout/stderr directly with metadata if requested
   - JSON format: Return complete execution result as formatted JSON
   - YAML format: Return execution result as YAML
   - Handle binary output detection and formatting
   - Show truncation warnings when output exceeds limits

6. **Environment Variable Parsing**:
   - Parse `KEY=VALUE` format from CLI arguments
   - Validate environment variable names and values
   - Apply security validation through existing workflow security functions
   - Handle parsing errors with helpful messages

7. **Exit Code Management**:
   - Return shell command exit code for human format
   - Return 0 for successful tool execution in JSON/YAML formats
   - Handle timeout errors appropriately
   - Use CLI exit code constants (EXIT_SUCCESS, EXIT_WARNING, EXIT_ERROR)

### Key Design Decisions

- **Reuse Existing MCP Tool**: Leverage the comprehensive `shell_execute` MCP tool rather than duplicating logic
- **Follow CLI Patterns**: Use the same argument parsing, MCP integration, and error handling patterns as existing commands (issue, memo, search)
- **Security Integration**: Apply the same security validation that the MCP tool uses
- **Output Consistency**: Provide both human-readable and machine-readable output formats like other CLI commands
- **Exit Code Semantics**: Mirror the shell command's exit code for intuitive CLI usage

### Benefits

- **No Code Duplication**: CLI directly uses the battle-tested MCP tool implementation
- **Consistent Security**: Same security controls apply whether using CLI or MCP
- **Unified Maintenance**: Updates to shell execution logic automatically benefit both interfaces
- **Rich Feature Set**: CLI inherits all MCP tool features (timeout, environment, security, output handling)
## Implementation Status: ✅ COMPLETE

The CLI integration for shell command execution has been successfully implemented and thoroughly tested. All requirements have been met.

### ✅ Completed Features

#### 1. **Shell Tool Registration** (`swissarmyhammer-cli/src/mcp_integration.rs`)
- ✅ Added `register_shell_tools` import
- ✅ Integrated shell tools into the CLI tool registry

#### 2. **CLI Command Definition** (`swissarmyhammer-cli/src/cli.rs`)
- ✅ Added `Shell` variant to `Commands` enum with comprehensive help text
- ✅ Created `ShellCommands` enum with `Execute` subcommand
- ✅ Defined `ShellOutputFormat` enum (human, json, yaml)
- ✅ Implemented all CLI parameters matching MCP tool schema:
  - `command` (required): Shell command string
  - `working_directory` (-C, --directory): Optional working directory
  - `timeout` (-t, --timeout): Timeout in seconds (default 300, max 1800)
  - `environment` (-e, --env): Environment variables as KEY=VALUE pairs
  - `format` (--format): Output format (human, json, yaml)
  - `show_metadata` (--show-metadata): Include execution metadata
  - `quiet` (-q, --quiet): Suppress command output

#### 3. **Shell CLI Handler** (`swissarmyhammer-cli/src/shell.rs`)
- ✅ Created comprehensive shell command handler
- ✅ Implemented environment variable parsing with proper validation
- ✅ Built MCP tool arguments using `CliToolContext`
- ✅ Created custom JSON extraction for shell responses (`extract_shell_json_response`)
- ✅ Implemented all three output formats (human, json, yaml)
- ✅ Added comprehensive timeout handling
- ✅ Proper exit code management (reflects shell command exit codes)
- ✅ Security validation through existing workflow security functions
- ✅ Added 6 unit tests for environment variable parsing

#### 4. **Main CLI Integration** (`swissarmyhammer-cli/src/main.rs`)
- ✅ Added shell module import
- ✅ Added shell command handling to main dispatcher
- ✅ Implemented `run_shell` function following existing patterns

#### 5. **Advanced Output Handling**
- ✅ **Human Format**: Direct stdout/stderr display with optional metadata
- ✅ **JSON Format**: Complete execution result as formatted JSON
- ✅ **YAML Format**: Complete execution result as YAML
- ✅ **Timeout Support**: Special handling for timeout responses across all formats
- ✅ **Binary Output**: Detection and safe formatting
- ✅ **Output Truncation**: Warnings when output exceeds limits
- ✅ **Quiet Mode**: Suppresses command output while preserving metadata

#### 6. **Error Handling and Security**
- ✅ Distinguishes between tool errors and shell command failures
- ✅ Security validation prevents dangerous commands
- ✅ Proper exit codes (shell command exit code in human format, tool success in JSON/YAML)
- ✅ Comprehensive timeout handling with partial output capture
- ✅ Environment variable validation and parsing

### ✅ Testing Results

All functionality has been thoroughly tested and verified:

#### Manual Testing Completed
- ✅ Basic command execution: `echo 'Hello, World!'`
- ✅ Command with metadata: `--show-metadata`
- ✅ All output formats: `--format human|json|yaml`
- ✅ Environment variables: `-e KEY=value`
- ✅ Working directory: `-C /tmp`
- ✅ Command failures: `ls /nonexistent` (proper exit code handling)
- ✅ Security validation: Commands with dangerous patterns blocked
- ✅ Timeout handling: `sleep 3` with `-t 1` (proper timeout response)
- ✅ Quiet mode: `--quiet` (output suppressed, metadata preserved)

#### Unit Tests
- ✅ 6 unit tests for environment variable parsing
- ✅ All existing CLI tests continue to pass
- ✅ No regressions in existing functionality

### ✅ Key Implementation Highlights

#### **Custom Timeout Handling**
The implementation correctly handles the special timeout response format from the MCP tool:
- First content item: Plain text timeout message
- Second content item: JSON timeout metadata
- Proper parsing and display across all output formats

#### **Sophisticated Error Handling**
- **Tool Errors**: Security validation, invalid parameters → Exit code 2
- **Shell Command Failures**: Non-zero exit codes → Exit with shell's exit code
- **Timeouts**: Commands that exceed timeout → Exit code 1

#### **Environment Variable Parsing**
- Supports `KEY=VALUE` format with equals signs in values
- Proper validation of variable names
- Clear error messages for invalid formats

#### **Security Integration**
- Leverages existing SwissArmyHammer security validation
- Same security controls apply whether using CLI or MCP
- Commands like `rm -rf /` and injection patterns are blocked

### ✅ Command Examples Working

```bash
# Basic execution
sah shell execute "echo 'Hello, World!'"

# With working directory and timeout
sah shell execute "pwd" -C /tmp -t 60

# With environment variables
sah shell execute "echo \$MY_VAR" -e MY_VAR=value -e DEBUG=true

# JSON output with metadata
sah shell execute "uname -a" --format json --show-metadata

# Quiet mode (no output, metadata only)
sah shell execute "echo test" --quiet --show-metadata

# Timeout handling
sah shell execute "sleep 5" -t 2  # Times out after 2 seconds
```

All examples execute correctly with expected behavior and proper exit codes.