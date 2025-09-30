# Git Changes Tool

List files that have changed on a branch relative to its parent branch, including uncommitted changes.

## Purpose

The Git Changes tool provides intelligent change detection for git branches, automatically determining the scope of changes based on branch type. This tool is essential for:
- Identifying which files have been modified in feature branches
- Understanding the complete file set for code review
- Tracking uncommitted work alongside committed changes
- Supporting workflow automation and change analysis

## Key Concepts

### Parent Branch Detection

The tool automatically determines whether a branch has a parent:
- **Feature/Issue branches** (e.g., `issue/feature-123`): Files changed since diverging from the parent branch
- **Main/trunk branches**: All tracked files in the repository (cumulative changes)

For branches starting with `issue/`, the tool uses merge-base calculation to find the parent branch automatically.

### Uncommitted Changes

The tool includes ALL uncommitted changes in its output:
- Staged modifications, additions, and deletions
- Unstaged modifications and deletions
- Renamed files
- Untracked files

This ensures you see the complete picture of what's changed, not just committed work.

## Parameters

- `branch` (required): Branch name to analyze
  - Type: string
  - Description: The name of the git branch to analyze for changes
  - Examples: `"main"`, `"issue/feature-123"`, `"develop"`

## Response Format

Returns a JSON object with the following structure:

```json
{
  "branch": "issue/feature-123",
  "parent_branch": "main",
  "files": [
    "src/main.rs",
    "README.md",
    "tests/integration_test.rs"
  ]
}
```

### Response Fields

- `branch` (string): The analyzed branch name
- `parent_branch` (string | null): The parent branch if detected, null for root branches like main
- `files` (array): Sorted, deduplicated list of file paths that have changed

## Examples

### Feature Branch with Parent Detection

Get changes on an issue branch (automatically detects parent):
```json
{
  "branch": "issue/feature-123"
}
```

Response:
```json
{
  "branch": "issue/feature-123",
  "parent_branch": "main",
  "files": [
    "src/feature.rs",
    "tests/feature_test.rs"
  ]
}
```

### Main Branch (All Tracked Files)

Get all files on main branch:
```json
{
  "branch": "main"
}
```

Response:
```json
{
  "branch": "main",
  "parent_branch": null,
  "files": [
    "Cargo.toml",
    "README.md",
    "src/main.rs",
    "src/lib.rs"
  ]
}
```

### Branch with Uncommitted Changes

When a branch has uncommitted changes, they are automatically included:
```json
{
  "branch": "issue/work-in-progress"
}
```

Response includes both committed and uncommitted files:
```json
{
  "branch": "issue/work-in-progress",
  "parent_branch": "main",
  "files": [
    "src/committed_change.rs",
    "src/uncommitted_edit.rs",
    "new_untracked_file.txt"
  ]
}
```

## Use Cases

### Code Review Preparation
Identify all files that need review before creating a pull request:
```json
{
  "branch": "issue/user-authentication"
}
```

### Change Impact Analysis
Understand which files have been touched in a feature branch:
```json
{
  "branch": "issue/refactor-database"
}
```

### Workflow Automation
Get the complete change set for automated testing or deployment:
```json
{
  "branch": "issue/api-endpoints"
}
```

### Repository Overview
List all tracked files in the main branch:
```json
{
  "branch": "main"
}
```

## Edge Cases

### Branch Without Parent
For branches that don't follow the `issue/` naming convention, the tool returns all tracked files (same as main branch behavior):
```json
{
  "branch": "develop",
  "parent_branch": null,
  "files": ["all", "tracked", "files"]
}
```

### Clean Branch
If a branch has no changes from its parent, the files array will be empty:
```json
{
  "branch": "issue/no-changes",
  "parent_branch": "main",
  "files": []
}
```

### Only Uncommitted Changes
If a branch exists but only has uncommitted changes, only those uncommitted files are returned:
```json
{
  "branch": "issue/draft-work",
  "parent_branch": "main",
  "files": ["uncommitted_file.txt"]
}
```

## Error Conditions

- **Git operations not available**: Returned when the tool is used outside a git repository
- **Invalid branch name**: Returned when the specified branch does not exist
- **Failed to get changed files**: Returned when git operations fail (e.g., corrupted repository)