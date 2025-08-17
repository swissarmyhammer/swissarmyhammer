Create new files or completely overwrite existing files.

## Parameters

- `file_path` (required): Absolute path for the new or existing file
- `content` (required): Complete file content to write

## Functionality

- Creates new files with specified content
- Overwrites existing files completely
- Creates parent directories if they don't exist
- Sets appropriate file permissions
- Validates file path and content

## Use Cases

- Creating new source files
- Generating configuration files
- Writing documentation or README files
- Creating test files or fixtures

## Examples

Create new file:
```json
{
  "file_path": "/home/user/project/src/new_module.rs",
  "content": "//! New module\n\npub fn hello() {\n    println!(\"Hello, world!\");\n}"
}
```

Overwrite existing file:
```json
{
  "file_path": "/home/user/project/config.toml",
  "content": "[database]\nurl = \"postgresql://localhost:5432/mydb\"\nmax_connections = 10\n"
}
```

## Returns

Returns confirmation that the file was created or overwritten successfully with file path and size information.