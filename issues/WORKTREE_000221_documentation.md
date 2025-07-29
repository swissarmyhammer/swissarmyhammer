# Documentation for Worktree-Based Workflow

## Overview
Update all documentation to reflect the new worktree-based issue workflow. This includes user guides, API documentation, examples, and migration notes.

## Implementation

### Update Issue Management Guide (`doc/src/issue-management.md`)

Add new section on worktree workflow:

```markdown
## Worktree-Based Issue Management

Swiss Army Hammer now uses Git worktrees to provide isolated workspaces for each issue. This approach offers several advantages:

- **Complete Isolation**: Each issue has its own working directory
- **Parallel Development**: Work on multiple issues simultaneously
- **Clean Context Switching**: No need to stash changes when switching issues
- **Simplified Workflow**: No branch switching in the main repository

### How It Works

When you start working on an issue, Swiss Army Hammer creates:
1. A branch named `issue/<issue_name>`
2. A worktree at `.swissarmyhammer/worktrees/issue-<issue_name>`

### Basic Workflow

1. **Create an Issue**
   ```bash
   swissarmyhammer issue create feature-auth --content "Implement authentication"
   ```

2. **Start Working**
   ```bash
   swissarmyhammer issue work 000001_feature-auth
   ```
   This creates a worktree at `.swissarmyhammer/worktrees/issue-000001_feature-auth/`

3. **Navigate to Worktree**
   ```bash
   cd .swissarmyhammer/worktrees/issue-000001_feature-auth/
   ```

4. **Make Changes**
   Work normally in the isolated worktree. All git operations work as expected.

5. **Complete and Merge**
   ```bash
   swissarmyhammer issue complete 000001_feature-auth
   swissarmyhammer issue merge 000001_feature-auth --delete-branch
   ```
   This merges changes and automatically cleans up the worktree.

### Managing Multiple Issues

You can work on multiple issues simultaneously:

```bash
# Start first issue
swissarmyhammer issue work 000001_feature-auth

# Start second issue (from main repo)
cd /path/to/main/repo
swissarmyhammer issue work 000002_bug-fix

# Check active worktrees
swissarmyhammer issue worktrees
```

### Tips and Best Practices

- Always run issue commands from the main repository directory
- Each worktree is a complete copy of your repository at that branch
- Commits made in a worktree are immediately visible when you merge
- The worktree directory is automatically cleaned up after merge
```

### Update MCP Protocol Documentation (`doc/src/mcp-protocol.md`)

Add worktree information to tool descriptions:

```markdown
### issue_work

Creates an isolated worktree for working on an issue.

**Request:**
```json
{
  "name": "issue_work",
  "arguments": {
    "name": "REFACTOR_000123_cleanup-code"
  }
}
```

**Response:**
```json
{
  "content": [{
    "type": "text",
    "text": "Created worktree for issue 'REFACTOR_000123_cleanup-code' at: /path/to/repo/.swissarmyhammer/worktrees/issue-REFACTOR_000123_cleanup-code\n\nTo start working:\n  cd /path/to/repo/.swissarmyhammer/worktrees/issue-REFACTOR_000123_cleanup-code"
  }]
}
```

The worktree provides complete isolation from the main repository, allowing parallel development on multiple issues.

### issue_merge

Merges an issue branch and cleans up its worktree.

**Request:**
```json
{
  "name": "issue_merge",
  "arguments": {
    "name": "REFACTOR_000123_cleanup-code",
    "delete_branch": true
  }
}
```

The merge operation automatically removes the associated worktree directory after successful merge.
```

### Add Troubleshooting Section (`doc/src/troubleshooting.md`)

```markdown
## Worktree Issues

### Worktree Already Exists

**Error:** "Worktree for issue 'X' already exists"

**Solution:** Navigate to the existing worktree:
```bash
cd .swissarmyhammer/worktrees/issue-X
```

### Cannot Merge from Within Worktree

**Error:** "Cannot merge worktree while inside it"

**Solution:** Change to the main repository before merging:
```bash
cd /path/to/main/repo
swissarmyhammer issue merge X
```

### Orphaned Worktrees

**Problem:** Worktree directories exist but aren't recognized by git

**Solution:** Clean up orphaned worktrees:
```bash
swissarmyhammer issue cleanup-worktrees
```

### Recovering from Corrupted Worktrees

If a worktree becomes corrupted:

1. Try automatic recovery:
   ```bash
   git worktree prune
   ```

2. Manual cleanup:
   ```bash
   rm -rf .swissarmyhammer/worktrees/issue-X
   git worktree prune
   swissarmyhammer issue work X  # Recreate
   ```
```

### Update README.md

Add worktree feature to main README:

```markdown
## Features

- **Isolated Workspaces**: Each issue gets its own git worktree for complete isolation
- **Parallel Development**: Work on multiple issues simultaneously without conflicts
- **Automatic Cleanup**: Worktrees are cleaned up automatically after merge
```

### Add Migration Guide (`doc/src/migration-worktree.md`)

```markdown
# Migrating to Worktree-Based Workflow

## Overview

The worktree-based workflow is the new default for Swiss Army Hammer. This guide helps you migrate existing issues.

## What's Changed

- `issue work` now creates a worktree instead of just switching branches
- `issue merge` automatically cleans up worktrees
- `issue current` shows active worktrees

## Migration Steps

### For Issues with Existing Branches

No action needed! The system automatically handles existing branches:

1. When you run `issue work` on an existing issue, it creates a worktree for the existing branch
2. The workflow continues normally from there

### Understanding the New Structure

```
your-repo/
├── .swissarmyhammer/
│   └── worktrees/          # New: Worktree directory
│       ├── issue-001/      # Isolated workspace for issue 001
│       └── issue-002/      # Isolated workspace for issue 002
├── .git/
└── ... (main repo files)
```

## Benefits

1. **No More Stashing**: Each issue has its own workspace
2. **Faster Context Switching**: Just `cd` to a different worktree
3. **Parallel Testing**: Run tests in multiple issues simultaneously
4. **Cleaner Git History**: No accidental commits to wrong branches

## Troubleshooting

See the [Troubleshooting Guide](./troubleshooting.md#worktree-issues) for common issues.
```

### Update Examples (`doc/examples/`)

Add worktree workflow example:

```bash
#!/bin/bash
# doc/examples/worktree-workflow.sh

# Example: Working on multiple issues with worktrees

echo "Creating first issue..."
swissarmyhammer issue create refactor-auth --content "Refactor authentication module"

echo "Creating second issue..."
swissarmyhammer issue create fix-validation --content "Fix input validation bug"

echo "Starting work on first issue..."
swissarmyhammer issue work 000001_refactor-auth
WORKTREE1=$(pwd)/.swissarmyhammer/worktrees/issue-000001_refactor-auth

echo "Starting work on second issue..."
swissarmyhammer issue work 000002_fix-validation
WORKTREE2=$(pwd)/.swissarmyhammer/worktrees/issue-000002_fix-validation

echo "Making changes in first worktree..."
cd "$WORKTREE1"
echo "Authentication refactored" > auth.txt
git add auth.txt
git commit -m "Refactor authentication"

echo "Making changes in second worktree..."
cd "$WORKTREE2"
echo "Validation fixed" > validation.txt
git add validation.txt
git commit -m "Fix validation"

echo "Completing and merging issues..."
cd /main/repo
swissarmyhammer issue complete 000001_refactor-auth
swissarmyhammer issue merge 000001_refactor-auth --delete-branch

swissarmyhammer issue complete 000002_fix-validation
swissarmyhammer issue merge 000002_fix-validation --delete-branch

echo "Workflow complete!"
```

## Dependencies
- All previous worktree implementation steps must be complete

## Testing
1. Verify all documentation is accurate
2. Test example scripts work correctly
3. Ensure troubleshooting steps resolve issues
4. Validate migration guide accuracy

## Context
This final step ensures users have comprehensive documentation for the new worktree-based workflow, including guides, examples, and troubleshooting information.