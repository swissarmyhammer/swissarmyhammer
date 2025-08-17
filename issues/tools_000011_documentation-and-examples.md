# Documentation and Examples for File Tools

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Create comprehensive documentation and examples for all file editing tools following established patterns.

## Documentation Tasks

### Tool Descriptions
- [ ] Complete `description.md` files for each tool following established patterns
- [ ] Include parameter descriptions, use cases, and examples
- [ ] Document security considerations and limitations
- [ ] Provide clear error condition explanations

### CLI Help Integration
- [ ] Comprehensive CLI help text for all commands
- [ ] Usage examples in help output
- [ ] Parameter validation error messages
- [ ] Troubleshooting guidance

### API Documentation
- [ ] Rust API documentation with `///` comments
- [ ] Module-level documentation with `//!` comments
- [ ] Code examples in documentation
- [ ] Integration pattern examples

## Examples and Use Cases

### Read Tool Examples
```bash
# Basic file reading
sah file read /path/to/file.rs

# Read specific lines
sah file read /path/to/large-file.txt --offset 100 --limit 50

# Read binary file (returns base64)
sah file read /path/to/image.png
```

### Write Tool Examples
```bash
# Create new file
sah file write /path/to/new-file.rs "fn main() { println!(\"Hello\"); }"

# Overwrite existing file
sah file write /path/to/config.toml "[section]\nkey = \"value\""
```

### Edit Tool Examples
```bash
# Replace single occurrence
sah file edit /path/to/file.rs "old_function_name" "new_function_name"

# Replace all occurrences
sah file edit /path/to/file.rs "TODO" "DONE" --replace-all
```

### Glob Tool Examples
```bash
# Find all Rust files
sah file glob "**/*.rs"

# Find files in specific directory
sah file glob "src/**/*.ts" --path /project/root

# Case sensitive matching
sah file glob "*Test*" --case-sensitive
```

### Grep Tool Examples
```bash
# Find function definitions
sah file grep "fn \w+\(" --type rust

# Search with context
sah file grep "error" --context-lines 3

# Count matches only
sah file grep "TODO" --output-mode count
```

## MCP Integration Examples
```json
// Read tool MCP call
{
  "tool": "file_read",
  "parameters": {
    "absolute_path": "/workspace/src/main.rs",
    "offset": 10,
    "limit": 20
  }
}

// Edit tool MCP call
{
  "tool": "file_edit", 
  "parameters": {
    "file_path": "/workspace/config.toml",
    "old_string": "debug = false",
    "new_string": "debug = true"
  }
}
```

## Integration Patterns Documentation
- [ ] Tool composition examples (Glob → Read → Edit workflows)
- [ ] Security best practices
- [ ] Performance optimization tips
- [ ] Common error handling patterns
- [ ] Workspace boundary considerations

## Documentation Structure
```
doc/src/
├── file-tools.md              # Overview and introduction
├── file-tools/
│   ├── read-tool.md          # Read tool documentation
│   ├── write-tool.md         # Write tool documentation  
│   ├── edit-tool.md          # Edit tool documentation
│   ├── glob-tool.md          # Glob tool documentation
│   ├── grep-tool.md          # Grep tool documentation
│   ├── cli-usage.md          # CLI command examples
│   ├── mcp-integration.md    # MCP protocol examples
│   ├── security.md           # Security considerations
│   └── troubleshooting.md    # Common issues and solutions
```

## Testing Documentation
- [ ] Document test patterns and approaches
- [ ] Provide examples for testing file operations
- [ ] Security testing guidelines
- [ ] Performance testing methodology

## Acceptance Criteria
- [ ] All tools have comprehensive description.md files
- [ ] CLI help text is complete and informative
- [ ] API documentation covers all public interfaces
- [ ] Examples demonstrate all major use cases
- [ ] Integration patterns are clearly documented
- [ ] Security considerations are thoroughly covered
- [ ] Documentation follows established patterns
- [ ] Examples are tested and verified to work