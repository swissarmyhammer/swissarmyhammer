Read file contents from the local filesystem with partial reading support.

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
