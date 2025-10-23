# File Operations

File tools provide comprehensive, secure file system access for AI assistants. All operations include validation, proper encoding handling, and atomic writes where applicable.

## Available Tools

### files_read

Read file contents with optional partial reading.

**Parameters:**
- `path` (required): Path to file (absolute or relative to working directory)
- `offset` (optional): Starting line number (1-based)
- `limit` (optional): Maximum number of lines to read

**Returns:**
- `content`: File contents as string
- `contentType`: Content type (`text` or `binary`)
- `encoding`: File encoding (e.g., `utf-8`)
- `linesRead`: Number of lines read
- `totalLines`: Total lines in file

**Example:**
```json
{
  "path": "Cargo.toml"
}
```

**Example with offset/limit:**
```json
{
  "path": "src/main.rs",
  "offset": 100,
  "limit": 50
}
```

### files_write

Write content to file atomically with encoding preservation.

**Parameters:**
- `file_path` (required): Absolute path to file
- `content` (required): Content to write

**Returns:**
- Confirmation with file path and size

**Example:**
```json
{
  "file_path": "/path/to/project/config.toml",
  "content": "[server]\nport = 3000\n"
}
```

**Features:**
- Atomic writes (write to temp, then rename)
- Creates parent directories if needed
- Preserves file encoding
- Backup on overwrite (configurable)

### files_edit

Perform precise string replacement in files.

**Parameters:**
- `file_path` (required): Absolute path to file
- `old_string` (required): Exact text to replace
- `new_string` (required): Replacement text
- `replace_all` (optional): Replace all occurrences (default: false)

**Returns:**
- File path
- Number of replacements made
- Bytes written
- Encoding detected
- Line ending format

**Example:**
```json
{
  "file_path": "/path/to/Cargo.toml",
  "old_string": "version = \"0.1.0\"",
  "new_string": "version = \"0.2.0\""
}
```

**Features:**
- Exact string matching (no regex)
- Preserves line endings (LF, CRLF, CR)
- Maintains file encoding
- Atomic operation
- Single or multiple replacements

### files_glob

Find files matching glob patterns with gitignore support.

**Parameters:**
- `pattern` (required): Glob pattern (e.g., `**/*.rs`, `src/**/*.ts`)
- `path` (optional): Directory to search (default: working directory)
- `case_sensitive` (optional): Case-sensitive matching (default: false)
- `respect_git_ignore` (optional): Honor .gitignore (default: true)

**Returns:**
- File count
- List of matching file paths (sorted by modification time)

**Example:**
```json
{
  "pattern": "**/*.rs"
}
```

**Pattern Examples:**
- `*.rs` - All Rust files in current directory
- `**/*.rs` - All Rust files recursively
- `src/**/*.{rs,toml}` - Rust and TOML files in src/
- `tests/**/test_*.rs` - Test files in tests/

### files_grep

Search file contents using ripgrep.

**Parameters:**
- `pattern` (required): Regular expression to search
- `path` (optional): File or directory to search
- `glob` (optional): Filter files by glob pattern
- `type` (optional): File type filter (js, py, rust, etc.)
- `case_insensitive` (optional): Case-insensitive search
- `context_lines` (optional): Lines of context around matches
- `output_mode` (optional): `content`, `files_with_matches`, or `count`

**Returns:**
Format depends on `output_mode`:
- `content`: Matches with file paths, line numbers, and content
- `files_with_matches`: List of files containing matches
- `count`: Match counts per file

**Example:**
```json
{
  "pattern": "fn\\s+\\w+\\s*\\(",
  "type": "rust",
  "output_mode": "content"
}
```

**Pattern Examples:**
- `error` - Find "error" in any file
- `fn\s+main` - Find function definitions
- `TODO|FIXME` - Find comment markers
- `impl\s+\w+` - Find trait implementations

## Common Use Cases

### Reading Configuration

```
Use files_read to read Cargo.toml
```

### Updating Dependencies

```
Use files_edit to update dependency version in Cargo.toml:
old: tokio = "1.0"
new: tokio = "1.35"
```

### Finding All Tests

```
Use files_glob with pattern "**/*_test.rs"
```

### Searching for TODOs

```
Use files_grep with pattern "TODO|FIXME"
```

### Batch File Updates

```
1. Use files_glob to find all target files
2. Use files_read to read each file
3. Use files_edit to make changes
4. Repeat for all files
```

## Security Features

### Path Validation

All paths are validated:
- Must be within working directory or allowed directories
- No directory traversal (`..` components)
- Symlinks resolved safely
- Absolute paths canonicalized

### Size Limits

Configurable limits prevent issues:
- Maximum file size (default: 10MB)
- Maximum search results (default: 1000)
- Timeout for operations

### Encoding Detection

Automatic encoding detection and handling:
- UTF-8 (most common)
- UTF-16 (LE/BE)
- Latin1/ASCII
- Binary files handled appropriately

### Atomic Operations

Write operations are atomic:
- Write to temporary file
- Validate content
- Rename to target (atomic on POSIX)
- Rollback on error

## Best Practices

### Use Relative Paths

```json
{
  "path": "src/main.rs"  // Good: relative to working directory
}
```

Instead of:
```json
{
  "path": "/home/user/project/src/main.rs"  // Avoid: absolute paths
}
```

### Read Large Files Partially

```json
{
  "path": "large_file.log",
  "offset": 1000,
  "limit": 100
}
```

### Use Specific Glob Patterns

```json
{
  "pattern": "src/**/*.rs"  // Good: specific
}
```

Instead of:
```json
{
  "pattern": "**/*"  // Avoid: too broad
}
```

### Validate Before Write

1. Use files_read to check current content
2. Make changes
3. Use files_write or files_edit
4. Verify with files_read

## Error Handling

Common errors and solutions:

**"Permission denied":**
- Check file permissions
- Verify user has write access
- Check directory is writable

**"File not found":**
- Verify path is correct
- Check working directory
- Ensure file exists

**"Invalid path":**
- Path contains directory traversal
- Path outside allowed directories
- Check for typos

**"File too large":**
- File exceeds max size limit
- Use partial reading with offset/limit
- Configure larger limit if needed

## Performance Tips

### 1. Use Glob for Finding Files

Glob is optimized and respects gitignore:
```
files_glob pattern="**/*.rs"
```

Instead of:
```
shell_execute command="find . -name '*.rs'"
```

### 2. Use Grep for Content Search

Grep uses ripgrep (very fast):
```
files_grep pattern="TODO" type="rust"
```

### 3. Read Large Files Partially

Only read what you need:
```json
{
  "offset": 1000,
  "limit": 100
}
```

### 4. Filter Early

Use glob and type filters to reduce file set:
```json
{
  "pattern": "TODO",
  "glob": "**/*.rs",
  "type": "rust"
}
```

## Next Steps

- **[Search Tools](search.md)** - Semantic code search
- **[Issue Management](issues.md)** - Track work items
