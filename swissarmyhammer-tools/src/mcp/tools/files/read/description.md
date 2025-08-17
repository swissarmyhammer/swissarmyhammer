Read and return file contents from the local filesystem with support for various file types.

## Parameters

- `absolute_path` (required): Full absolute path to the file to read
- `offset` (optional): Starting line number for partial reading
- `limit` (optional): Maximum number of lines to read

## Functionality

- Validates file path (must be absolute and within workspace)
- Supports text files, images, PDFs, and other file types
- Enables partial file reading via offset/limit for large files
- Provides error handling for missing or inaccessible files
- Respects workspace boundaries and ignore patterns

## Use Cases

- Reading source code files for analysis
- Examining configuration files  
- Viewing documentation or README files
- Reading specific sections of large files

## Examples

Read entire file:
```json
{
  "absolute_path": "/home/user/project/src/main.rs"
}
```

Read specific section of large file:
```json
{
  "absolute_path": "/home/user/project/logs/debug.log",
  "offset": 100,
  "limit": 50
}
```

## Returns

Returns the file contents with metadata including file type, size, and encoding information.