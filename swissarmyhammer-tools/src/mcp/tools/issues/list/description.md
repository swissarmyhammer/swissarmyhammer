List all available issues with their status and metadata.

## Parameters

- `show_completed` (optional): Include completed issues (default: false)
- `show_active` (optional): Include active issues (default: true)
- `format` (optional): Output format - "table", "json", or "markdown" (default: "table")

## Examples

```json
{
  "show_completed": true,
  "format": "json"
}
```

## Returns

Returns formatted list of issues with names, status, creation dates, and file paths.
