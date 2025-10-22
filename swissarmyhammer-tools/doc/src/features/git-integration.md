# Git Integration

SwissArmyHammer provides git integration for tracking file changes with branch detection and parent branch tracking.

## Overview

The git integration helps AI assistants understand what has changed in a codebase, enabling context-aware development and accurate change tracking.

## Available Tool

### git_changes

List files that have changed on a branch relative to its parent branch, including uncommitted changes.

**Parameters:**
- `branch` (required): Branch name to analyze

**Returns:**
- Branch name
- Parent branch (if detected)
- Array of changed file paths

**Example:**
```json
{
  "branch": "issue/feature-123"
}
```

## How It Works

### Parent Branch Detection

For `issue/*` branches, the parent branch is automatically detected:

```
issue/feature-123 → parent: main
issue/bugfix-456 → parent: develop
```

For main/trunk branches, returns all tracked files.

### Change Detection

The tool detects:

1. **Committed Changes**: Files changed in commits on the branch
2. **Staged Changes**: Files added to the staging area
3. **Unstaged Changes**: Modified files not yet staged
4. **Untracked Files**: New files not yet added to git

### Change Scope

Changes are relative to the parent branch:

```
main (parent)
  └── issue/feature
      ├── file1.rs (modified)
      ├── file2.rs (new)
      └── file3.rs (uncommitted)
```

All three files are reported as changes.

## Use Cases

### Review Changes Before Commit

Check what has changed:

```json
{
  "branch": "issue/add-auth"
}
```

Returns:
```json
{
  "branch": "issue/add-auth",
  "parent_branch": "main",
  "changed_files": [
    "src/auth.rs",
    "src/middleware.rs",
    "tests/auth_test.rs"
  ]
}
```

### Understand Branch Scope

See all work done on a feature branch:

```json
{
  "branch": "issue/refactor-api"
}
```

### Generate Change Summary

Get list of changed files for documentation:

```json
{
  "branch": "current-branch"
}
```

Use output to:
- Generate commit messages
- Create PR descriptions
- Document changes

### Identify Test Files

Filter changed files to find tests:

```json
{
  "branch": "issue/feature"
}
```

Then filter results for `*_test.rs` or `*_test.py`.

## Integration Patterns

### Pre-Commit Review

Before committing:

1. Run `git_changes` to see what changed
2. Review each file
3. Verify all intended changes present
4. Check for unintended changes

### PR Preparation

When creating a PR:

1. Get changed files with `git_changes`
2. Generate summary of changes
3. Identify affected areas
4. List test files modified

### Context Loading

For AI assistance:

1. Run `git_changes` to identify scope
2. Read changed files with `files_read`
3. Provide context to AI
4. Work on related changes

### Change Validation

After refactoring:

1. Run `git_changes` to see impact
2. Verify expected files changed
3. Check for unexpected changes
4. Validate test coverage

## Branch Naming Conventions

### Automatic Parent Detection

The tool recognizes these patterns:

- `issue/*` → parent: `main` or `develop`
- `feature/*` → parent: `main`
- `bugfix/*` → parent: `main`
- `hotfix/*` → parent: `main`

### Custom Patterns

For other patterns, parent detection may not work automatically. Ensure your branch names follow conventions or work from a known branch.

## Working with Main Branch

When analyzing main/trunk branches:

```json
{
  "branch": "main"
}
```

Returns all tracked files, as there's no parent to compare against.

## Uncommitted Changes

The tool includes uncommitted changes:

- **Staged**: Files added with `git add`
- **Unstaged**: Modified but not staged
- **Untracked**: New files not yet tracked

This gives a complete picture of work in progress.

## Best Practices

### Branch Naming

1. **Use Prefixes**: `issue/`, `feature/`, `bugfix/`
2. **Be Descriptive**: `issue/add-user-authentication` not `issue/123`
3. **Consistent Pattern**: Stick to one convention
4. **Avoid Spaces**: Use hyphens or underscores

### Change Tracking

1. **Regular Checks**: Run `git_changes` frequently
2. **Before Commits**: Review changes before committing
3. **Before PRs**: Verify scope before creating PR
4. **After Merges**: Check what landed in main

### Workflow Integration

1. **Start of Work**: Check what's already changed
2. **During Work**: Monitor scope creep
3. **Before Commit**: Final review
4. **Before PR**: Generate description

## Limitations

### No Diff Content

The tool returns file paths only, not the actual diff content. Use `files_read` to read file contents or standard git commands to see diffs.

### Parent Detection Limits

Parent branch detection works for common patterns. Custom branch patterns may need explicit parent specification.

### Performance with Large Changes

For branches with thousands of changed files, results may be large. Consider filtering results client-side.

### No Commit History

The tool doesn't provide commit messages or history. Use git commands for detailed history.

## Combining with Other Tools

### With files_read

```json
// 1. Get changed files
{"branch": "issue/feature"}

// 2. Read each changed file
{"path": "/workspace/src/changed_file.rs"}
```

### With files_grep

```json
// 1. Get changed files
{"branch": "issue/feature"}

// 2. Search within changed files
{"pattern": "TODO", "path": "src/changed_file.rs"}
```

### With issue_update

```json
// 1. Get changed files
{"branch": "issue/feature"}

// 2. Update issue with progress
{
  "name": "feature",
  "content": "\n\n## Files Changed\n- file1.rs\n- file2.rs",
  "append": true
}
```

### With search_query

```json
// 1. Get changed files
{"branch": "issue/feature"}

// 2. Search for similar code
{"query": "authentication pattern"}
```

## Examples

### Simple Branch Check

```json
{
  "branch": "issue/add-logging"
}
```

Response:
```json
{
  "branch": "issue/add-logging",
  "parent_branch": "main",
  "changed_files": [
    "src/logger.rs",
    "src/main.rs"
  ]
}
```

### Feature Branch Review

```json
{
  "branch": "feature/user-dashboard"
}
```

Response:
```json
{
  "branch": "feature/user-dashboard",
  "parent_branch": "main",
  "changed_files": [
    "src/dashboard.rs",
    "src/components/user_widget.rs",
    "tests/dashboard_test.rs",
    "static/dashboard.css"
  ]
}
```

## Troubleshooting

### No Parent Branch Detected

**Issue:** Parent branch not detected.

**Solution:** 
- Check branch naming follows conventions
- Verify parent branch exists
- Ensure git repository is valid

### No Changes Reported

**Issue:** No changed files returned.

**Solution:**
- Verify you're on the correct branch
- Check if there are actually changes: `git status`
- Ensure working directory is a git repository

### Wrong Parent Branch

**Issue:** Wrong parent branch detected.

**Solution:**
- Check branch naming convention
- Verify parent branch reference
- Ensure branch was created from correct parent

## Next Steps

- [Issue Management](./issue-management.md): Track work with issues
- [File Operations](./file-operations.md): Read and modify files
- [Workflow Execution](./workflow-execution.md): Automate workflows
