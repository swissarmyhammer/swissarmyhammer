Content-based search with ripgrep. Supports full regex syntax.

## Examples

```json
{"pattern": "fn\\s+\\w+\\s*\\(", "type": "rust"}
{"pattern": "TODO|FIXME", "path": "src/"}
{"pattern": "import.*from", "type": "ts", "output_mode": "content"}
```

## Returns

Returns matches with file paths, line numbers, and content. Format depends on `output_mode`.
