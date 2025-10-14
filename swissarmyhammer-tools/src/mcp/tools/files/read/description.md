Read file contents from the local filesystem with partial reading support.

## Parameters

- `path` (required): Path to the file to read (absolute or relative)
- `offset` (optional): Starting line number for partial reading (1-based, max 1,000,000)
- `limit` (optional): Maximum number of lines to read (1-100,000 lines)

## Examples

```json
{
  "path": "/workspace/src/main.rs"
}
```

```json
{
  "path": "logs/application.log",
  "offset": 1000,
  "limit": 100
}
```

## Returns

Returns file content (text or base64 for binary), content type, encoding, and line counts.
