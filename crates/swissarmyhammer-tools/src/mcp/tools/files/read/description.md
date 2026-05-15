Read file contents from the local filesystem with partial reading support.

Paths can be absolute or relative to the working directory. Use `offset` and `limit` for partial reads of large files.

## Examples

```json
{"path": "/workspace/src/main.rs"}
{"path": "logs/application.log", "offset": 1000, "limit": 100}
```

## Returns

Returns file content (text or base64 for binary), content type, encoding, and line counts.
