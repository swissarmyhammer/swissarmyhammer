# File Operations

SwissArmyHammer provides comprehensive file system operations with security validation, atomic writes, and proper encoding handling.

## Overview

The file operations tools enable AI assistants to safely interact with the file system while respecting security boundaries and maintaining data integrity.

## Available Tools

### files_read

Read file contents from the local filesystem with partial reading support.

**Parameters:**
- `path` (required): Path to the file (absolute or relative)
- `offset` (optional): Starting line number (1-based, max 1,000,000)
- `limit` (optional): Maximum number of lines to read (1-100,000)

**Example:**
```json
{
  "path": "/workspace/src/main.rs"
}
```

**Features:**
- Supports partial reading for large files
- Automatic encoding detection (UTF-8, UTF-16, etc.)
- Returns base64 for binary files
- Line-numbered output for easy reference

### files_write

Write content to files with atomic operations, creating new files or overwriting existing ones.

**Parameters:**
- `file_path` (required): Absolute path for the file
- `content` (required): Complete file content to write

**Example:**
```json
{
  "file_path": "/workspace/src/new_module.rs",
  "content": "//! New module\n\npub fn hello() {\n    println!(\"Hello, world!\");\n}"
}
```

**Features:**
- Atomic write operations (write to temp, then move)
- Creates parent directories if needed
- Preserves file permissions
- UTF-8 encoding with BOM detection

### files_edit

Perform precise string replacements in files with atomic operations.

**Parameters:**
- `file_path` (required): Absolute path to the file
- `old_string` (required): Exact text to replace
- `new_string` (required): Replacement text
- `replace_all` (optional): Replace all occurrences (default: false)

**Example:**
```json
{
  "file_path": "/workspace/src/config.rs",
  "old_string": "const DEBUG: bool = true;",
  "new_string": "const DEBUG: bool = false;"
}
```

**Features:**
- Exact string matching
- Atomic operations
- Line ending preservation (LF, CRLF)
- Encoding preservation
- Reports number of replacements made

### files_glob

Fast file pattern matching with .gitignore support.

**Parameters:**
- `pattern` (required): Glob pattern (e.g., `**/*.rs`, `src/**/*.ts`)
- `path` (optional): Directory to search (default: current directory)
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore (default: true)

**Example:**
```json
{
  "pattern": "**/*.rs"
}
```

**Features:**
- Fast globbing with walkdir
- Respects .gitignore patterns by default
- Returns files sorted by modification time
- Supports complex glob patterns

### files_grep

Content-based search with ripgrep for fast text searching.

**Parameters:**
- `pattern` (required): Regular expression pattern
- `path` (optional): File or directory to search
- `glob` (optional): Glob pattern to filter files (e.g., `*.js`)
- `type` (optional): File type filter (e.g., `js`, `py`, `rust`)
- `case_insensitive` (optional): Case-insensitive search (default: false)
- `context_lines` (optional): Context lines around matches (default: 0)
- `output_mode` (optional): `content`, `files_with_matches`, or `count`

**Example:**
```json
{
  "pattern": "fn\\s+\\w+\\s*\\(",
  "type": "rust"
}
```

**Features:**
- Lightning-fast search with ripgrep
- Full regex support
- File type filtering
- Context lines
- Multiple output modes

## Security Features

All file operations include security validation:

- **Path Validation**: Ensures paths are within working directory
- **Symlink Detection**: Prevents symlink escape attacks
- **Parent Directory Creation**: Safe directory creation with validation
- **Atomic Operations**: Prevents partial writes and data corruption

## Best Practices

### Reading Files

- Use `offset` and `limit` for large files to avoid memory issues
- Check file encoding in response for proper handling
- Use line numbers from output for precise editing

### Writing Files

- Always use absolute paths
- Verify parent directory exists or will be created
- Consider using `files_edit` for small changes instead of full rewrites

### Editing Files

- Make `old_string` as specific as possible to avoid unintended replacements
- Use `replace_all: true` for renaming variables across a file
- Preserve formatting and whitespace in replacements

### Pattern Matching

- Use `files_glob` for finding files by name patterns
- Use `files_grep` for finding files by content
- Combine both for complex searches

## Common Use Cases

### Finding Configuration Files

```json
{
  "pattern": "**/config.{json,yaml,toml}"
}
```

### Finding Function Definitions

```json
{
  "pattern": "^\\s*(pub\\s+)?fn\\s+\\w+",
  "type": "rust",
  "output_mode": "content"
}
```

### Updating Version Numbers

```json
{
  "file_path": "/workspace/Cargo.toml",
  "old_string": "version = \"0.1.0\"",
  "new_string": "version = \"0.2.0\""
}
```

### Reading Large Log Files

```json
{
  "path": "/var/log/application.log",
  "offset": 1000,
  "limit": 100
}
```

## Error Handling

File operations return detailed error information:

- **File Not Found**: Clear message with attempted path
- **Permission Denied**: Indicates permission issues
- **Invalid Pattern**: Reports glob or regex syntax errors
- **Security Violation**: Reports path traversal attempts

## Performance Considerations

- **files_glob**: Very fast, optimized for large directories
- **files_grep**: Extremely fast, uses ripgrep
- **files_read**: Efficient streaming for large files
- **files_write**: Atomic operations have small overhead
- **files_edit**: Must read entire file into memory

## Next Steps

- [Semantic Search](./semantic-search.md): Learn about code search capabilities
- [Issue Management](./issue-management.md): Track work with file-based issues
