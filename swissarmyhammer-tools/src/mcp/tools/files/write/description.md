# File Write Tool

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