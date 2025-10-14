Update the content of an existing issue.

## Parameters

- `name` (required): Issue name to update
- `content` (required): New markdown content for the issue
- `append` (optional): If true, append to existing content instead of replacing (default: false)

## Examples

Replace issue content:
```json
{
  "name": "REFACTOR_000123_cleanup-code",
  "content": "# Updated issue content\\n\\nNew requirements...",
  "append": false
}
```

## Returns

Returns confirmation that the issue has been updated.
