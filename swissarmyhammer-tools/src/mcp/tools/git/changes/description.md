List files that have changed on a branch relative to its parent branch, including uncommitted changes.

## Parameters

- `branch` (required): Branch name to analyze

## Examples

```json
{
  "branch": "issue/feature-123"
}
```

## Returns

Returns branch name, parent branch (if detected), and array of changed file paths. For issue/* branches, automatically detects parent. For main/trunk branches, returns all tracked files.
