# files_edit

Perform precise string replacements in existing files with atomic operations, encoding preservation, and comprehensive validation.

## Purpose

Make targeted changes to existing files using exact string matching and replacement. All operations are atomic - if any error occurs, the original file remains unchanged.

## Parameters

- `file_path` (required): Absolute path to the file to modify
- `old_string` (required): Exact text to replace (cannot be empty)
- `new_string` (required): Replacement text (must be different from old_string)
- `replace_all` (optional): Replace all occurrences (default: false)

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

## CLI Usage

```bash
# Single replacement
sah files edit --file-path ./config.rs --old-string "debug = true" --new-string "debug = false"

# Replace all occurrences
sah files edit --file-path ./main.rs --old-string "oldName" --new-string "newName" --replace-all

# Multi-line replacement
sah files edit --file-path ./README.md --old-string $'## Old Title\nOld content' --new-string $'## New Title\nNew content'
```

## Use Cases

- **Code Refactoring**: Update variable names, function signatures, or constants
- **Configuration Updates**: Modify config values while preserving format
- **Bug Fixes**: Make targeted changes to specific code sections
- **Documentation Updates**: Update text content with precise replacements
- **Batch Processing**: Use replace_all for consistent updates across file

## Security Features

- **Atomic Operations**: Either all changes succeed or none are applied
- **Encoding Preservation**: Maintains original file encoding (UTF-8, UTF-16, etc.)
- **Line Ending Preservation**: Preserves LF, CRLF, or mixed line endings
- **Metadata Preservation**: Maintains file permissions and timestamps
- **Workspace Boundaries**: Restricted to current working directory

## Response Format

Returns comprehensive information including:
- File path and confirmation of successful edit
- Number of replacements made
- Bytes written to the file
- Detected file encoding
- Line ending format preserved
- Whether file metadata was successfully preserved