Write content to files with atomic operations, creating new files or overwriting existing ones.

## Parameters

- `file_path` (required): Absolute path for the new or existing file
- `content` (required): Complete file content to write

## Examples

```json
{
  "file_path": "/workspace/src/new_module.rs",
  "content": "//! New module\\n\\npub fn hello() {\\n    println!(\\\"Hello, world!\\\");\\n}"
}
```

## Returns

Returns confirmation with file path and size information.
