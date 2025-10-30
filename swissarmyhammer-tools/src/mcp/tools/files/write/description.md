Write content to files with atomic operations, creating new files or overwriting existing ones.

## Examples

```json
{
  "file_path": "/workspace/src/new_module.rs",
  "content": "//! New module\\n\\npub fn hello() {\\n    println!(\\\"Hello, world!\\\");\\n}"
}
```

## Returns

Returns confirmation with file path and size information.
