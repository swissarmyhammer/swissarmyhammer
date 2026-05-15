Perform precise string replacements in files with atomic operations.

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
