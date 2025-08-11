# Issue List

List all available issues with their status and metadata.

## Parameters

- `show_completed` (optional): Include completed issues in the list (default: false)
- `show_active` (optional): Include active issues in the list (default: true)
- `format` (optional): Output format - "table", "json", or "markdown" (default: "table")

## Examples

List all active issues:
```json
{}
```

List all issues including completed:
```json
{
  "show_completed": true,
  "show_active": true
}
```

List issues in JSON format:
```json
{
  "format": "json"
}
```

## Returns

Returns a formatted list of issues matching the specified criteria, including their names, status, creation dates, and file paths.