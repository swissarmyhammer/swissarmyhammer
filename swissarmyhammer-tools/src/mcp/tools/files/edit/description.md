Perform precise string replacements in existing files with atomic operations, encoding preservation, and comprehensive validation.

## Parameters

- `file_path` (required): Absolute path to the file to modify
- `old_string` (required): Exact text to replace (cannot be empty)
- `new_string` (required): Replacement text (must be different from old_string)
- `replace_all` (optional): Replace all occurrences (default: false)

## Enhanced Functionality

### Atomic Operations
- Uses temporary files in the same directory for atomic writes
- All-or-nothing replacement with automatic rollback on failure
- Prevents data corruption from interrupted operations
- Automatic cleanup of temporary files on any error

### File Encoding & Format Preservation
- Detects and preserves original file encoding (UTF-8, UTF-16, etc.)
- Maintains line ending styles (Unix LF, Windows CRLF, Mac CR)
- Handles Unicode content correctly including emojis and international characters
- Preserves Byte Order Mark (BOM) when present

### Metadata Preservation
- Maintains file permissions and ownership
- Preserves file timestamps (access and modification times)
- Keeps extended attributes where supported by filesystem

### Comprehensive Validation
- Validates file existence before any modifications
- Ensures old_string exists in file content
- For single replacements: validates old_string is unique in file
- For replace_all: handles multiple occurrences safely
- Comprehensive path validation and security checks

### Enhanced Response Information
- Detailed success messages including:
  - Number of replacements made
  - Bytes written to file
  - Detected encoding information
  - Line ending format preserved
  - Metadata preservation status

## Use Cases

- **Code Refactoring**: Update variable names, function signatures, or constants
- **Configuration Updates**: Modify config values while preserving format
- **Bug Fixes**: Make targeted changes to specific code sections
- **Documentation Updates**: Update text content with precise replacements
- **Batch Processing**: Use replace_all for consistent updates across file

## Examples

### Single Replacement
```json
{
  "file_path": "/home/user/project/src/config.rs",
  "old_string": "const DEBUG: bool = true;",
  "new_string": "const DEBUG: bool = false;"
}
```

### Replace All Occurrences
```json
{
  "file_path": "/home/user/project/src/main.rs", 
  "old_string": "old_variable_name",
  "new_string": "new_variable_name",
  "replace_all": true
}
```

### Unicode Content Replacement
```json
{
  "file_path": "/home/user/docs/readme.txt",
  "old_string": "Hello 🌍!",
  "new_string": "Hello 🚀!"
}
```

## Returns

Returns comprehensive success information including:
- File path and confirmation of successful edit
- Number of replacements made
- Bytes written to the file
- Detected file encoding (e.g., "UTF-8", "UTF-16")
- Line ending format preserved (e.g., "LF", "CRLF", "Mixed")
- Whether file metadata was successfully preserved

## Error Handling

The tool provides detailed error messages for common scenarios:
- File not found or inaccessible
- Old string not found in file content
- Multiple occurrences found when single replacement expected
- Identical old_string and new_string values
- Permission or encoding issues
- Path validation failures

All operations are atomic - if any error occurs, the original file remains unchanged and no temporary files are left behind.