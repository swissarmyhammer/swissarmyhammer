Perform precise string replacements in files with atomic operations.

## Parameters

- `file_path` (required): Absolute path to the file to modify
- `old_string` (required): Exact text to replace
- `new_string` (required): Replacement text
- `replace_all` (optional): Replace all occurrences (default: false)

## Examples

```json
{
  "file_path": "/home/user/project/src/config.rs",
  "old_string": "const DEBUG: bool = true;",
  "new_string": "const DEBUG: bool = false;"
}
```

## Returns

Returns file path, number of replacements, bytes written, encoding, and line ending format.
