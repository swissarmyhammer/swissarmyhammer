Switch to a work branch for the specified issue (creates branch issue/<issue_name> if needed).

## Parameters

- `name` (required): Issue name to work on

## Examples

Start working on an issue:
```json
{
  "name": "REFACTOR_000123_cleanup-code"
}
```

## Returns

Returns confirmation that you've switched to the work branch for the specified issue. If the branch doesn't exist, it will be created automatically with the pattern `issue/<issue_name>`.