# File Write Tool

Write content to files with atomic operations, creating new files or completely overwriting existing ones.

## Purpose

The File Write tool provides safe, atomic file writing operations for creating new files or completely replacing existing file content. It ensures data integrity through atomic operations and comprehensive security validation.

## Parameters

- `file_path` (required): Absolute path for the new or existing file
- `content` (required): Complete file content to write

## Functionality

### Core Features
- **Atomic Operations**: Uses temporary file strategy (write to temp, then rename) to ensure atomicity
- **New File Creation**: Creates new files with specified content
- **File Overwriting**: Completely replaces existing file content
- **Parent Directory Creation**: Automatically creates parent directories if they don't exist
- **Security Validation**: Comprehensive path validation and workspace boundary enforcement
- **Encoding Handling**: Proper UTF-8 encoding validation and handling
- **Permission Management**: Sets appropriate file permissions

### Security Measures
- **Workspace Boundaries**: All file paths must be within configured workspace
- **Path Validation**: Absolute paths required with comprehensive security checks
- **Directory Traversal Protection**: Prevents path traversal attacks (../ sequences)
- **File Permission Validation**: Ensures write permissions before operations
- **Content Validation**: Validates content encoding and format

### Atomic Write Process
1. **Validation**: Comprehensive path and content validation
2. **Temporary File**: Create temporary file in target directory with `.tmp` suffix
3. **Content Write**: Write complete content to temporary file
4. **Atomic Rename**: Rename temporary file to target filename (atomic operation)
5. **Cleanup**: Remove temporary file on any failure

## Use Cases

### Development Workflows
- Creating new source files from templates
- Generating configuration files
- Writing build scripts and automation files
- Creating test files and fixtures

### Content Generation
- Writing documentation files (README, API docs)
- Generating data files (JSON, CSV, XML)
- Creating template files for code generation
- Writing log files and reports

## Examples

Create new file:
```json
{
  "file_path": "/workspace/src/new_module.rs",
  "content": "//! New module\n\npub fn hello() {\n    println!(\"Hello, world!\");\n}"
}
```

Overwrite existing file:
```json
{
  "file_path": "/workspace/config.toml", 
  "content": "[database]\nurl = \"postgresql://localhost:5432/mydb\"\nmax_connections = 10\n"
}
```

Write configuration file:
```json
{
  "file_path": "/workspace/config/settings.json",
  "content": "{\n  \"environment\": \"development\",\n  \"debug\": true,\n  \"database\": {\n    \"host\": \"localhost\",\n    \"port\": 5432\n  }\n}"
}
```

## Error Handling

### Input Validation
- **Empty Path**: File path cannot be empty or whitespace
- **Relative Path**: File path must be absolute, not relative
- **Invalid Characters**: Path contains control characters or null bytes

### Security Violations
- **Workspace Boundary**: Path is outside configured workspace boundaries
- **Path Traversal**: Path contains dangerous traversal sequences
- **Blocked Patterns**: Path matches configured security patterns

### File System Errors
- **Permission Denied**: Insufficient permissions to write
- **Parent Directory**: Parent directory missing and cannot be created
- **Read-Only File**: Target file exists but is read-only
- **Disk Full**: Insufficient disk space for operation

## Returns

Returns confirmation that the file was created or overwritten successfully with file path and size information.

### Success Response
```json
{
  "content": [{"type": "text", "text": "Successfully wrote 156 bytes to /workspace/src/main.rs"}],
  "is_error": false
}
```

### Error Response
```json
{
  "content": [{"type": "text", "text": "Permission denied accessing: /workspace/protected/file.txt"}],
  "is_error": true
}
```

## Integration Notes

- **Tool Chaining**: Often used after `files_read` to modify content
- **Pattern Matching**: Combine with `files_glob` for batch operations
- **Content Search**: Use with `files_grep` to create filtered files
- **Version Control**: Creates new files that can be tracked by Git