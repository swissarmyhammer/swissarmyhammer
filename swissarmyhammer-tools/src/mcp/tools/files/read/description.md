Read file contents from the local filesystem with partial reading support.

## Security

This tool implements comprehensive security measures:

- **Path Validation**: All file paths undergo security validation with path traversal protection
- **Workspace Boundaries**: Enforces workspace directory restrictions when configured to prevent unauthorized access outside the project
- **Permission Checking**: Validates read permissions before attempting file access
- **Audit Logging**: All file access attempts are logged for security monitoring
- **Resource Limits**: Configurable offset/limit parameters prevent excessive resource usage

Paths can be absolute or relative to the current working directory. The tool automatically resolves symlinks and canonicalizes paths to detect and prevent directory traversal attacks.

## Examples

Read entire file:
```json
{
  "path": "/workspace/src/main.rs"
}
```

Read partial file with offset and limit:
```json
{
  "path": "logs/application.log",
  "offset": 1000,
  "limit": 100
}
```

## Returns

Returns file content (text or base64 for binary), content type, encoding, and line counts.
