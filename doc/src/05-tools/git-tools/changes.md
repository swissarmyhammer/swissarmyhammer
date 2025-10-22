# git_changes

List files that have changed on a branch relative to its parent branch, including uncommitted changes.

## Purpose

The `git_changes` tool provides a programmatic way to determine which files are part of the "scope of work" for a given branch. This is essential for workflows that need to:

- Focus on relevant files for code review
- Run selective tests based on changes
- Generate documentation for modified modules
- Analyze impact of pull requests

## Parameters

### branch (required)

The name of the git branch to analyze.

- **Type**: String
- **Required**: Yes
- **Example**: `"issue/feature-123"`, `"main"`, `"develop"`

## Behavior

### Feature/Issue Branches

For branches that diverged from a parent (e.g., `issue/*`, `feature/*`):

1. Identifies the parent branch automatically
2. Uses git diff to find all files changed since divergence
3. Adds uncommitted changes (staged, unstaged, untracked)
4. Returns the deduplicated list

### Main/Trunk Branches

For main branches (e.g., `main`, `master`, `develop`):

1. Returns only uncommitted changes
2. Includes staged, unstaged, and untracked files
3. Does not show historical changes (as there's no clear divergence point)

## Response Format

```json
{
  "branch": "issue/feature-123",
  "parent_branch": "main",
  "files": [
    "src/lib.rs",
    "src/git/mod.rs",
    "tests/git_tests.rs",
    "README.md"
  ]
}
```

### Fields

- `branch`: The analyzed branch name
- `parent_branch`: The detected parent branch (null for main branches)
- `files`: Array of file paths that have changed (sorted and deduplicated)

## Examples

### MCP Usage (Claude Code)

```json
{
  "tool": "git_changes",
  "parameters": {
    "branch": "issue/feature-123"
  }
}
```

### CLI Usage

```bash
# Analyze current branch
sah git changes --branch $(git branch --show-current)

# Analyze specific branch
sah git changes --branch issue/feature-123

# Get changed files for review
sah git changes --branch $BRANCH | jq -r '.files[]'
```

### Workflow Usage

```yaml
### analyze
Determine which files need review
**Actions**:
  - tool: git_changes
    branch: $CURRENT_BRANCH
    save_as: changed_files
**Next**: review
```

## Use Cases

### Selective Testing

```bash
# Get changed test files
changed_files=$(sah git changes --branch $BRANCH | jq -r '.files[] | select(test("_test.rs$"))')

# Run only those tests
for file in $changed_files; do
  cargo nextest run --test $(basename $file .rs)
done
```

### Code Review Focus

```bash
# List changed files with line counts
sah git changes --branch $PR_BRANCH | jq -r '.files[]' | while read file; do
  lines=$(wc -l < "$file")
  echo "$file: $lines lines"
done
```

### Documentation Updates

```bash
# Find changed Rust files that need doc updates
sah git changes --branch $BRANCH | \
  jq -r '.files[] | select(endswith(".rs"))' | \
  xargs -I {} echo "Update docs for {}"
```

## Edge Cases

### No Changes

If the branch has no changes relative to its parent:

```json
{
  "branch": "issue/feature-123",
  "parent_branch": "main",
  "files": []
}
```

### New Branch

For a newly created branch with no commits:

```json
{
  "branch": "feature/new",
  "parent_branch": "main",
  "files": []
}
```

### Detached HEAD

If the repository is in detached HEAD state, the tool will analyze changes relative to the last reachable branch point.

## Error Handling

The tool returns errors for:

- **Not a git repository**: Working directory is not inside a git repository
- **Branch does not exist**: Specified branch name is invalid
- **Repository corruption**: Git database is corrupted or inaccessible

## Performance

The tool uses libgit2 for efficient git operations:

- **Fast**: No external git process spawning
- **Memory efficient**: Streams file lists without loading full diffs
- **Reliable**: Direct repository access without shell command parsing

## Best Practices

### Branch Naming Conventions

Use consistent branch naming for automatic parent detection:

```bash
# Good - automatically detects parent
git checkout -b issue/feature-123
git checkout -b feature/user-auth
git checkout -b bugfix/crash-on-startup

# Less ideal - may require manual parent tracking
git checkout -b mybranch
```

### Include Uncommitted Changes

The tool includes uncommitted changes by design. To get only committed changes, commit or stash your work first:

```bash
# Save uncommitted work
git stash

# Get only committed changes
sah git changes --branch $BRANCH

# Restore uncommitted work
git stash pop
```

### Large Repositories

For very large repositories with extensive history, the initial analysis may take a few seconds. Subsequent calls are faster due to git's internal caching.
