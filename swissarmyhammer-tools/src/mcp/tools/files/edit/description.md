Perform precise string replacements in existing files.

## Parameters

- `file_path` (required): Absolute path to the file to modify
- `old_string` (required): Exact text to replace
- `new_string` (required): Replacement text
- `replace_all` (optional): Replace all occurrences (default: false)

## Functionality

- Performs exact string matching and replacement
- Maintains file encoding and line endings
- Validates that old_string exists and is unique (unless replace_all is true)
- Provides atomic operations (all or nothing replacement)
- Preserves file permissions and metadata

## Use Cases

- Modifying specific code sections
- Updating variable names or function signatures
- Fixing bugs with targeted changes
- Refactoring code with precise replacements

## Examples

Single replacement:
```json
{
  "file_path": "/home/user/project/src/config.rs",
  "old_string": "const DEBUG: bool = true;",
  "new_string": "const DEBUG: bool = false;"
}
```

Replace all occurrences:
```json
{
  "file_path": "/home/user/project/src/main.rs", 
  "old_string": "old_variable_name",
  "new_string": "new_variable_name",
  "replace_all": true
}
```

## Returns

Returns confirmation of the replacement operation with details about the number of replacements made.