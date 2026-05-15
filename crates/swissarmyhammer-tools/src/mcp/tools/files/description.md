File operations for reading, writing, editing, and searching files.

## Operations

- **read file**: Read file contents from the local filesystem
- **write file**: Create new files or overwrite existing files with atomic operations
- **edit file**: Perform precise string replacements in existing files
- **glob files**: Fast file pattern matching with advanced filtering and sorting
- **grep files**: Content-based search using ripgrep for fast text searching

## Examples

```json
{"op": "read file", "path": "/src/main.rs"}
```

```json
{"op": "write file", "file_path": "/src/config.rs", "content": "// config"}
```

```json
{"op": "edit file", "file_path": "/src/main.rs", "old_string": "old_fn", "new_string": "new_fn"}
```

```json
{"op": "glob files", "pattern": "**/*.rs"}
```

```json
{"op": "grep files", "pattern": "TODO", "path": "/src"}
```
