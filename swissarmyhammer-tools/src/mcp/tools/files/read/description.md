# File Read Tool

Read and return file contents from the local filesystem with support for various file types and partial reading capabilities.

## Purpose

The File Read tool provides secure, validated file reading operations with comprehensive workspace boundary enforcement and partial reading support. It handles text files, binary content encoding, and large file processing efficiently.

## Parameters

- `absolute_path` (required): Full absolute path to the file to read
- `offset` (optional): Starting line number for partial reading (1-based, max 1,000,000)
- `limit` (optional): Maximum number of lines to read (1-100,000 lines)

## Enhanced Functionality

### Security & Validation
- **Absolute Path Requirement**: All paths must be absolute to prevent confusion and security issues
- **Workspace Boundary Enforcement**: Validates all paths are within configured workspace boundaries
- **Path Traversal Protection**: Prevents directory traversal attacks using `../` sequences
- **Comprehensive Path Validation**: Uses enhanced security validation framework
- **Permission Checking**: Verifies read permissions before attempting file access

### File Type Support
- **Text Files**: Direct UTF-8 content reading with encoding preservation
- **Binary Files**: Base64 encoding for images, executables, and other binary formats
- **Large Files**: Efficient partial reading without loading entire file into memory
- **Empty Files**: Graceful handling of zero-length files
- **Special Files**: Proper handling of symlinks, device files, and other special file types

### Partial Reading Capabilities
- **Line-Based Offset**: Start reading from specific line number (1-based indexing)
- **Configurable Limits**: Read up to 100,000 lines in a single request
- **Memory Efficient**: Processes large files without excessive memory usage
- **Boundary Validation**: Prevents excessive offset/limit values that could cause performance issues

### Error Handling & Logging
- **Structured Error Messages**: Clear, actionable error messages for all failure scenarios
- **Security Audit Logging**: All file access attempts logged for security monitoring
- **Permission Error Details**: Specific guidance for permission-related failures
- **Path Resolution Errors**: Detailed feedback for invalid or inaccessible paths

## Use Cases

### Development Workflows
- Reading source code files for analysis and review
- Examining configuration files and environment settings
- Viewing build scripts, makefiles, and automation files
- Reading test files and fixture data

### Documentation and Content
- Viewing README files, documentation, and markdown content
- Reading API documentation and specification files
- Accessing changelog and release note files
- Examining license and legal documents

### Large File Processing
- Reading specific sections of log files without loading entire content
- Processing large CSV or data files with offset/limit pagination
- Examining specific ranges of configuration or data files
- Incremental processing of large text documents

### Binary Content Access
- Reading binary files with automatic base64 encoding
- Accessing image metadata and content
- Reading compiled executables and libraries for analysis
- Processing archive and compressed file content

## Examples

### Basic File Reading
Read complete source file:
```json
{
  "absolute_path": "/workspace/src/main.rs"
}
```

Read configuration file:
```json
{
  "absolute_path": "/workspace/config/settings.toml"
}
```

### Large File Processing
Read specific section of large log file:
```json
{
  "absolute_path": "/workspace/logs/application.log",
  "offset": 1000,
  "limit": 100
}
```

Start reading from line 50:
```json
{
  "absolute_path": "/workspace/data/large_dataset.csv",
  "offset": 50
}
```

Read first 20 lines only:
```json
{
  "absolute_path": "/workspace/README.md",
  "limit": 20
}
```

### Binary File Reading
Read binary file (returns base64):
```json
{
  "absolute_path": "/workspace/assets/logo.png"
}
```

Read executable file:
```json
{
  "absolute_path": "/workspace/target/release/application"
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

## Error Handling

### Input Validation Errors
- **Empty Path**: `absolute_path cannot be empty`
- **Relative Path**: `File path must be absolute, not relative`
- **Invalid Offset**: `offset must be less than 1,000,000 lines`
- **Invalid Limit**: `limit must be greater than 0 and less than or equal to 100,000 lines`

### Security Validation Errors
- **Workspace Boundary**: `Path is outside workspace boundaries`
- **Path Traversal**: `Path contains blocked pattern '../'`
- **Permission Denied**: `Permission denied accessing: /path/to/file`

### File System Errors
- **File Not Found**: `File not found: /path/to/missing/file`
- **Read Permission**: `Insufficient permissions to read file`
- **Invalid File Type**: `Cannot read special file type`
- **File Too Large**: `File exceeds maximum readable size`

## Performance Characteristics

- **Memory Efficient**: Streams content without loading entire files
- **Large File Support**: Handles multi-gigabyte files with offset/limit
- **Fast Access**: Optimized path validation and security checks
- **Concurrent Safe**: Thread-safe operations for multiple simultaneous reads

## Security Considerations

- **Path Validation**: All paths undergo comprehensive security validation
- **Workspace Boundaries**: Strict enforcement of workspace access limits
- **Audit Logging**: All file access attempts logged for security monitoring
- **Permission Checking**: Validates read permissions before file access
- **Attack Prevention**: Protection against path traversal and other file system attacks

## Integration Notes

- **Tool Chaining**: Often used before `files_edit` to examine content before modifications
- **Pattern Matching**: Combine with `files_glob` to read multiple matched files
- **Content Search**: Use with `files_grep` to read files containing specific patterns
- **Batch Operations**: Can be used in workflows to process multiple files sequentially