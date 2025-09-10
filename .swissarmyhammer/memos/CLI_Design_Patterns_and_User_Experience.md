# CLI Design Patterns and User Experience

## Command Structure

### Subcommand Organization
- Use verb-noun pattern for commands: `sah create memo`, `sah list issues`
- Group related functionality under common subcommands
- Keep command names short but descriptive
- Use consistent naming across similar operations

### Parameter Design
- Use long-form flags with short aliases: `--verbose/-v`, `--output/-o`
- Make required parameters positional when unambiguous
- Use consistent flag names across commands
- Provide sensible defaults for optional parameters

### Input/Output Patterns
- Accept input from stdin when appropriate
- Support multiple output formats (JSON, YAML, table)
- Use colors and formatting to improve readability
- Respect NO_COLOR environment variable

## User Experience

### Error Messages
- Provide actionable error messages
- Suggest corrections for common mistakes
- Include context about what the tool was trying to do
- Use consistent error format across commands

### Progress Indication
- Show progress for long-running operations
- Use progress bars for deterministic tasks
- Provide status updates for network operations
- Allow cancellation with Ctrl+C

### Help and Documentation
- Generate help text from command definitions
- Provide examples in help output
- Include common usage patterns
- Keep help text concise but complete

## Configuration

### Configuration Files
- Support standard configuration locations
- Use TOML format for configuration files
- Allow environment variable overrides
- Provide configuration validation and error reporting

### Environment Variables
- Follow standard naming conventions (UPPERCASE_WITH_UNDERSCORES)
- Use consistent prefixes (SAH_*)
- Document all supported environment variables
- Provide defaults for all configuration values

### Workspace Detection
- Automatically detect project boundaries
- Use .swissarmyhammer directory as workspace marker
- Support nested workspace configurations
- Provide explicit workspace override options

## Output Formatting

### Structured Output
- Support JSON output for programmatic use
- Use consistent field names across different commands
- Include metadata in structured output
- Provide both human and machine readable formats

### Human-Readable Output
- Use tables for tabular data
- Apply consistent column formatting
- Use colors to highlight important information
- Provide sorting and filtering options

### Logging
- Use structured logging with appropriate levels
- Include request/response IDs for tracing
- Log to stderr to avoid polluting output
- Support verbose and quiet modes

## Integration Patterns

### MCP Integration
- Expose CLI functionality through MCP tools
- Maintain consistent parameter naming
- Handle MCP-specific error formats
- Support both CLI and MCP usage patterns

### External Tool Integration
- Support piping input/output to other tools
- Use exit codes consistently
- Follow UNIX conventions for tool behavior
- Support batch operations for scripting

### IDE Integration
- Provide language server capabilities where appropriate
- Support editor-specific file formats
- Integrate with common development workflows
- Provide machine-readable output for tooling