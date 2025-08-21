Perform precise string replacements in existing files with atomic operations, encoding preservation, and comprehensive validation.

All operations are atomic - if any error occurs, the original file remains unchanged and no temporary files are left behind.

## Parameters

- `file_path` (required): Absolute path to the file to modify
- `old_string` (required): Exact text to replace (cannot be empty)
- `new_string` (required): Replacement text (must be different from old_string)
- `replace_all` (optional): Replace all occurrences (default: false)

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


