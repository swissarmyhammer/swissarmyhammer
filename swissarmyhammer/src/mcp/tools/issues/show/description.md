# Issue Show

Display details of a specific issue by name.

## Parameters

- `name` (required): Name of the issue to show. Use `"current"` to show the issue for the current git branch. Use `"next"` to show the next pending issue.
- `raw` (optional): Show raw content only without formatting (default: false)

## Examples

Show issue with formatted display:
```json
{
  "name": "FEATURE_000123_user-auth"
}
```

Show raw issue content only:
```json
{
  "name": "FEATURE_000123_user-auth",
  "raw": true
}
```

Show current issue based on git branch:
```json
{
  "name": "current"
}
```

Show next pending issue:
```json
{
  "name": "next"
}
```

## Returns

Returns the issue details including status, creation date, file path, and content. When `raw` is true, returns only the raw markdown content.

When using `"current"` as the name:
- If on an issue branch, returns the current issue details
- If not on an issue branch, returns a message indicating the current branch
- If git operations are not available, returns an appropriate error message

When using `"next"` as the name:
- Returns the next pending issue details (alphabetically first pending issue)
- If no pending issues exist, returns a message indicating all issues are completed
- Uses the same selection logic as the original `issue_next` tool