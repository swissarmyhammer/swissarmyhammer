# Git Changes

List files that have changed on a branch relative to its parent branch.

## Parameters

- `branch` (required): Branch name to analyze

## Examples

Get changes on issue branch:
```json
{
  "branch": "issue/feature-123"
}
```

Get all files on main branch:
```json
{
  "branch": "main"
}
```

## Returns

Returns the branch name, parent branch (if applicable), and list of changed file paths.