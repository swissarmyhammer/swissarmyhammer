Merge the work branch for an issue back to the main branch.

## Parameters

- `name` (required): Issue name to merge
- `delete_branch` (optional): Whether to delete the branch after merging (default: false)

## Examples

Merge an issue and keep the branch:
```json
{
  "name": "REFACTOR_000123_cleanup-code"
}
```

Merge an issue and delete the branch:
```json
{
  "name": "REFACTOR_000123_cleanup-code",
  "delete_branch": true
}
```

## Returns

Returns confirmation that the issue work branch has been merged back to the main branch, and whether the branch was deleted if requested.