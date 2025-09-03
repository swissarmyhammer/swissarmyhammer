# File Tools

File tools provide comprehensive file system operations with security controls and workspace boundary enforcement. All file operations respect git repository boundaries and include comprehensive validation.

## Available Tools

- **files_read** - Read file contents with optional partial reading
- **files_write** - Create new files or completely overwrite existing ones  
- **files_edit** - Make precise string replacements in existing files
- **files_glob** - Find files using glob patterns with gitignore support
- **files_grep** - Search file contents using regular expressions

## Security Features

- **Workspace Boundaries** - Operations restricted to current working directory and subdirectories
- **Path Validation** - Prevents directory traversal and unsafe path operations
- **Atomic Operations** - File modifications are atomic (all-or-nothing)
- **Encoding Detection** - Automatic handling of UTF-8, UTF-16, and other encodings
- **Binary File Support** - Base64 encoding for binary content

## Usage Patterns

### CLI Usage
```bash
# Read a file
sah files read --absolute-path ./src/main.rs

# Write a new file
sah files write --file-path ./output.txt --content "Hello World"

# Edit existing file
sah files edit --file-path ./config.toml --old-string "debug = false" --new-string "debug = true"

# Find files by pattern
sah files glob --pattern "**/*.rs"

# Search file contents
sah files grep --pattern "TODO" --output-mode content
```

### MCP Integration
Tools are automatically available in Claude Code:
```
Use files_read to examine the main.rs file
Use files_glob to find all TypeScript files  
Use files_edit to update the configuration
```

### Workflow Usage
```yaml
# In workflow actions
- ReadConfig: Execute files_read with absolute_path="./config.toml" 
- FindSources: Execute files_glob with pattern="src/**/*.rs"
- UpdateVersion: Execute files_edit with file_path="./Cargo.toml" old_string="version = \"1.0.0\"" new_string="version = \"1.1.0\""
```