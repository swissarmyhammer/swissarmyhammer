# File Read Tool

Read and return file contents from the local filesystem with support for various file types and partial reading capabilities.

## Purpose

The File Read tool provides secure, validated file reading operations with comprehensive workspace boundary enforcement and partial reading support. It handles text files, binary content encoding, and large file processing efficiently.

## Parameters

- `path` (required): Path to the file to read (absolute or relative to current working directory)
- `offset` (optional): Starting line number for partial reading (1-based, max 1,000,000)
- `limit` (optional): Maximum number of lines to read (1-100,000 lines)

## Examples

### Basic File Reading
Read complete source file with absolute path:
```json
{
  "path": "/workspace/src/main.rs"
}
```

Read configuration file with relative path:
```json
{
  "path": "config/settings.toml"
}
```

Read file in current directory:
```json
{
  "path": "./README.md"
}
```

### Large File Processing
Read specific section of large log file with absolute path:
```json
{
  "path": "/workspace/logs/application.log",
  "offset": 1000,
  "limit": 100
}
```

Start reading from line 50 using relative path:
```json
{
  "path": "data/large_dataset.csv",
  "offset": 50
}
```

Read first 20 lines of current directory file:
```json
{
  "path": "README.md",
  "limit": 20
}
```

### Binary File Reading
Read binary file with absolute path (returns base64):
```json
{
  "path": "/workspace/assets/logo.png"
}
```

Read executable file with relative path:
```json
{
  "path": "target/release/application"
}
```

## Response Format

### Success Response
```json
{
  "content": [{"type": "text", "text": "Successfully read file content"}],
  "is_error": false,
  "file_content": "actual file content here...",
  "metadata": {
    "file_path": "/workspace/src/main.rs",
    "content_length": 2048,
    "content_type": "text",
    "encoding": "UTF-8",
    "lines_read": 50,
    "total_file_lines": 150
  }
}
```

### Binary File Response
```json
{
  "content": [{"type": "text", "text": "Binary file encoded as base64"}],
  "is_error": false,
  "file_content": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==",
  "metadata": {
    "file_path": "/workspace/logo.png",
    "content_type": "binary",
    "encoding": "base64",
    "file_size": 1024
  }
}
```