# SwissArmyHammer Coding Standards

## Architecture Guidelines

### File Loading and Resolution
- DO NOT duplicate the file loading and resolution logic between commands
- Use `PromptResolver` for loading prompts from standard locations (builtin/user/local)
- Use `WorkflowResolver` for loading workflows from standard locations
- Standard locations follow precedence: Builtin → User → Local (later overrides earlier)

### Storage Patterns
- Use the appropriate storage backend for your use case:
  - `MemoryStorage` for temporary/test scenarios
  - `FileSystemStorage` for persistent storage
  - `VirtualFileSystem` when you need layered file access

### Error Handling
- Use `anyhow::Result` for functions that can fail
- Provide context with `.context()` for better error messages
- Never panic in library code - return errors instead
- Use `tracing` for logging, not `println!` or `eprintln!`

## Security Considerations

### Path Validation
- Always validate file paths to prevent directory traversal attacks
- Use the security module's validation functions for paths
- Never trust user-provided paths without validation

### Resource Limits
- Limit file sizes when reading user-provided content
- Use the security module's complexity validation for workflows

## CLI Specific Guidelines

### Command Structure
- Commands should have a single, clear purpose
- Commands should never duplicate logic of an available MCP tool, instead calling the tool
- Use consistent parameter names across commands
- Provide helpful error messages with suggestions
- Support both human-readable and machine-readable output formats

### Backward Compatibility
- Take no steps for backward compatibility

## Workflow and Prompt Guidelines

### Validation
- All prompts must have required fields (title/name, description)
- Validate Liquid template syntax
- Check that template variables match declared arguments
- Workflows must have valid state machines

### Standard Locations
- Builtin: Embedded in the binary
- User: `~/.swissarmyhammer/`
- Local: `./.swissarmyhammer/` (in current directory or parents)

## Performance Considerations

### Lazy Loading
- Load resources only when needed
- Use iterators instead of collecting into vectors when possible
- Cache expensive computations appropriately

### Memory Usage
- Be mindful of memory usage with large workflow collections
- Consider streaming for large file operations
- Clean up resources promptly

## Version Control

### Commit Messages
- Use clear, descriptive commit messages
- Follow conventional commit format when applicable
- Reference issue numbers in commits
