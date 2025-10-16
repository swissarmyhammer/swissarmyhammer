Display details of a specific issue by name.

## Parameters

- `name` (required): Name of the issue to show. Use "next" for the next pending issue.
- `raw` (optional): Show raw content only without formatting (default: false)

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
