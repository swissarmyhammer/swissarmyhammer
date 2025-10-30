Display details of a specific issue by name.

## Examples

Show a specific issue by name:
```json
{
  "name": "FEATURE_000123_user-auth"
}
```

Show the next pending issue:
```json
{
  "name": "next"
}
```

## Returns

Returns issue details including status, creation date, file path, and content. When raw=true, returns only markdown content.
