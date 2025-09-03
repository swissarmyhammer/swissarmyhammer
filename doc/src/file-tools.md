# File Tools

The SwissArmyHammer file tools provide a comprehensive suite of secure file manipulation and search capabilities for AI-assisted development environments. These tools are designed to work together as a cohesive system for file management in development workflows.

## Overview

The file tools consist of five core operations that provide essential file system functionality:

- **[Read Tool](#read-tool)**: Secure file reading with partial content support
- **[Write Tool](#write-tool)**: Atomic file creation and overwriting
- **[Edit Tool](#edit-tool)**: Precise string replacement operations  
- **[Glob Tool](#glob-tool)**: Fast file pattern matching and discovery
- **[Grep Tool](#grep-tool)**: High-performance content search with ripgrep

All tools implement comprehensive security validation, workspace boundary enforcement, and structured error handling for safe operation in development environments.

## Security Framework

### Path Validation
- **Absolute Path Requirement**: All file paths must be absolute to prevent confusion and security issues
- **Workspace Boundary Enforcement**: All paths validated to be within configured workspace directories
- **Path Traversal Protection**: Blocks dangerous sequences like `../` to prevent directory traversal attacks
- **Pattern Blocking**: Configurable blocking of malicious path patterns and control characters

### Permission Checking
- Read permission validation before file access attempts
- Write permission checking for file creation and modification operations
- Directory creation permission validation for parent directories
- Read-only file detection and appropriate error handling

### Audit Logging
- All file access attempts logged for security monitoring
- Structured logging with file paths, operations, and results
- Security violation logging for workspace boundary breaches
- Performance metrics logging for monitoring and optimization

## Tool Composition Patterns

### Read → Edit Workflow
Examine file content before making targeted modifications:

```json
// 1. Read file to understand current content
{
  "tool": "files_read",
  "parameters": {
    "absolute_path": "/workspace/src/config.rs"
  }
}

// 2. Make precise edit based on current content
{
  "tool": "files_edit", 
  "parameters": {
    "file_path": "/workspace/src/config.rs",
    "old_string": "debug = false",
    "new_string": "debug = true"
  }
}
```

### Glob → Read Batch Processing
Discover and process multiple files matching patterns:

```json
// 1. Find all test files
{
  "tool": "files_glob",
  "parameters": {
    "pattern": "**/*test*.rs"
  }
}

// 2. Read each discovered file (iterate over results)
{
  "tool": "files_read",
  "parameters": {
    "absolute_path": "/workspace/src/lib_test.rs"
  }
}
```

### Grep → Edit Targeted Changes
Search for specific content and make targeted modifications:

```json
// 1. Find files containing deprecated API usage
{
  "tool": "files_grep",
  "parameters": {
    "pattern": "old_api_function\\(",
    "output_mode": "files_with_matches"
  }
}

// 2. Update each file with new API
{
  "tool": "files_edit",
  "parameters": {
    "file_path": "/workspace/src/module.rs",
    "old_string": "old_api_function(",
    "new_string": "new_api_function(",
    "replace_all": true
  }
}
```

### Write → Verify Workflow
Create files and verify content was written correctly:

```json
// 1. Create new configuration file
{
  "tool": "files_write",
  "parameters": {
    "file_path": "/workspace/config/new_feature.toml",
    "content": "[feature]\nenabled = true\nversion = \"1.0\""
  }
}

// 2. Verify content was written correctly
{
  "tool": "files_read",
  "parameters": {
    "absolute_path": "/workspace/config/new_feature.toml"
  }
}
```

## Read Tool

Secure file reading with support for text files, binary content, and partial reading of large files.

### Key Features
- Comprehensive security validation and workspace boundary enforcement
- Partial reading support with line-based offset and limit parameters
- Automatic binary file detection with base64 encoding
- Memory-efficient processing of large files
- Structured audit logging for security monitoring

### Usage Examples

**Basic File Reading:**
```json
{
  "tool": "files_read",
  "parameters": {
    "absolute_path": "/workspace/src/main.rs"
  }
}
```

**Large File Processing:**
```json
{
  "tool": "files_read", 
  "parameters": {
    "absolute_path": "/workspace/logs/application.log",
    "offset": 1000,
    "limit": 100
  }
}
```

**Binary File Access:**
```json
{
  "tool": "files_read",
  "parameters": {
    "absolute_path": "/workspace/assets/logo.png"
  }
}
```

## Write Tool

Atomic file creation and overwriting with comprehensive security validation.

### Key Features
- Atomic write operations using temporary file strategy
- Parent directory creation when needed
- Content size validation (max 10MB)
- UTF-8 encoding validation and proper handling
- Comprehensive security path validation

### Usage Examples

**Create New File:**
```json
{
  "tool": "files_write",
  "parameters": {
    "file_path": "/workspace/src/new_module.rs",
    "content": "//! New module\n\npub fn hello() {\n    println!(\"Hello, world!\");\n}"
  }
}
```

**Configuration File Creation:**
```json
{
  "tool": "files_write",
  "parameters": {
    "file_path": "/workspace/config/settings.toml",
    "content": "[database]\nurl = \"postgresql://localhost:5432/mydb\"\nmax_connections = 10"
  }
}
```

## Edit Tool  

Precise string replacement operations with atomic write guarantees.

### Key Features
- Exact string matching and replacement with validation
- Single or multiple occurrence replacement modes
- File encoding and line ending preservation
- Atomic operations with automatic rollback on failure
- Metadata preservation (permissions, timestamps)

### Usage Examples

**Single Replacement:**
```json
{
  "tool": "files_edit",
  "parameters": {
    "file_path": "/workspace/src/config.rs", 
    "old_string": "const DEBUG: bool = true;",
    "new_string": "const DEBUG: bool = false;"
  }
}
```

**Replace All Occurrences:**
```json
{
  "tool": "files_edit",
  "parameters": {
    "file_path": "/workspace/src/main.rs",
    "old_string": "old_variable_name", 
    "new_string": "new_variable_name",
    "replace_all": true
  }
}
```

## Glob Tool

Fast file pattern matching with advanced filtering and .gitignore support.

### Key Features
- Standard glob pattern support with recursive matching
- Full .gitignore integration using the `ignore` crate
- Case-sensitive and case-insensitive matching options
- Result limiting to prevent memory exhaustion (max 10,000 files)
- Git repository boundary awareness

### Usage Examples

**Find All Source Files:**
```json
{
  "tool": "files_glob",
  "parameters": {
    "pattern": "**/*.rs"
  }
}
```

**Directory-Scoped Search:**
```json
{
  "tool": "files_glob",
  "parameters": {
    "pattern": "**/*.{js,ts}",
    "path": "/workspace/src",
    "case_sensitive": true
  }
}
```

**Complex Pattern Matching:**
```json
{
  "tool": "files_glob",
  "parameters": {
    "pattern": "**/*{test,spec}.{js,ts}",
    "respect_git_ignore": true
  }
}
```

## Grep Tool

High-performance content search with ripgrep integration and intelligent fallback.

### Key Features
- Dual-engine architecture (ripgrep primary, regex fallback)
- Full regular expression support with validation
- File type filtering and glob pattern matching
- Context line extraction around matches
- Binary file detection and exclusion

### Usage Examples

**Find Function Definitions:**
```json
{
  "tool": "files_grep",
  "parameters": {
    "pattern": "fn\\s+\\w+\\s*\\(",
    "type": "rust",
    "output_mode": "content"
  }
}
```

**Search with Context:**
```json
{
  "tool": "files_grep", 
  "parameters": {
    "pattern": "TODO|FIXME",
    "case_insensitive": true,
    "context_lines": 2,
    "output_mode": "content"
  }
}
```

**Count Pattern Occurrences:**
```json
{
  "tool": "files_grep",
  "parameters": {
    "pattern": "error",
    "output_mode": "count"
  }
}
```

## Error Handling

All file tools provide comprehensive error handling with structured error messages:

### Input Validation Errors
- Empty or whitespace-only paths
- Relative paths when absolute paths are required
- Invalid parameter values (negative offsets, zero limits)
- Content size limit violations

### Security Validation Errors
- Workspace boundary violations
- Path traversal attack attempts
- Blocked path patterns detected
- Permission denied scenarios

### File System Errors
- File not found or inaccessible
- Read/write permission issues
- Parent directory missing or inaccessible
- Disk space or resource limitations

## Performance Considerations

### Memory Management
- Partial reading support for large files (offset/limit parameters)
- Streaming content processing to minimize memory usage
- Result limiting in glob operations (10,000 file maximum)
- Binary file detection to prevent unnecessary processing

### Security Performance
- Efficient path validation algorithms
- Cached workspace boundary calculations
- Optimized pattern matching for dangerous sequences
- Minimal overhead from audit logging

### Concurrent Operations
- Thread-safe operations for multiple simultaneous file access
- Atomic write operations prevent data corruption
- Lock-free read operations for maximum concurrency
- Resource cleanup guarantees prevent leaks

## Best Practices

### Security Best Practices
1. Always use absolute paths to prevent confusion
2. Implement workspace boundaries to limit file access scope
3. Monitor audit logs for security violations
4. Validate all user-provided file paths before operations
5. Use atomic operations for critical file modifications

### Performance Best Practices  
1. Use offset/limit parameters for large file processing
2. Leverage glob patterns for efficient file discovery
3. Use appropriate ripgrep file type filters for faster searches
4. Batch related operations to minimize overhead
5. Monitor file operation performance metrics

### Development Workflow Integration
1. Use Read → Edit patterns for safe file modifications
2. Combine Glob and Grep for comprehensive code analysis
3. Implement Write → Verify workflows for critical file creation
4. Use structured error handling for robust automation
5. Leverage tool composition for complex file processing tasks

## CLI Integration

The file tools integrate seamlessly with the SwissArmyHammer CLI:

```bash
# File operations through CLI
sah file read /path/to/file.rs
sah file write /path/to/new-file.rs "content"
sah file edit /path/to/file.rs "old" "new"
sah file glob "**/*.rs"
sah file grep "function" --type rust
```

## MCP Protocol Integration

All file tools are available through the MCP protocol for AI assistant integration:

- **Tool Names**: `files_read`, `files_write`, `files_edit`, `files_glob`, `files_grep`
- **Parameter Validation**: JSON schema validation for all parameters
- **Response Format**: Structured JSON responses with success/error indicators
- **Error Handling**: MCP-compatible error types and messages
- **Async Support**: Full async/await compatibility for concurrent operations

The file tools provide a robust, secure foundation for file system operations in AI-assisted development environments, with comprehensive documentation, extensive testing, and production-ready security features.