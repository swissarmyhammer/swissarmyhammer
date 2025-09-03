# files_write

Write content to files with atomic operations, creating new files or completely overwriting existing ones.

## Purpose

The File Write tool provides safe, atomic file writing operations for creating new files or completely replacing existing file content. It ensures data integrity through atomic operations and comprehensive security validation.

## Parameters

- `file_path` (required): Absolute path for the new or existing file
- `content` (required): Complete file content to write

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

## CLI Usage

```bash
# Create new file
sah files write --file-path ./src/new_module.rs --content "pub fn hello() { println!(\"Hello\"); }"

# Write configuration
sah files write --file-path ./config.toml --content "[app]\nname = \"myapp\""

# Write from stdin
echo "Hello World" | sah files write --file-path ./output.txt --content -
```

## Response Format

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

## Security Features

- **Atomic Operations**: Writes are atomic - either complete successfully or leave original file unchanged
- **Path Validation**: Absolute paths required, prevents directory traversal
- **Workspace Boundaries**: Restricted to current working directory and subdirectories
- **Permission Checks**: Validates write permissions before attempting operation
- **Backup Safety**: Creates temporary files during write process for data integrity

## Use Cases

- **Configuration Management**: Creating or updating configuration files
- **Code Generation**: Writing generated source code files
- **Documentation**: Creating markdown files and documentation
- **Data Export**: Writing processed data to output files
- **Template Instantiation**: Creating files from templates with substituted values

## Behavior Notes

- **Complete Replacement**: This tool completely replaces file content - use `files_edit` for partial modifications
- **Directory Creation**: Parent directories are not created automatically - they must exist
- **Encoding**: UTF-8 encoding is used by default for text content
- **Binary Content**: Binary data should be base64 encoded in the content parameter
- **File Permissions**: New files inherit default permissions from the system

## Error Handling

Common error scenarios:
- Permission denied (file or directory not writable)
- Path outside workspace boundaries
- Parent directory does not exist
- Disk space insufficient
- File system errors (read-only filesystem, etc.)