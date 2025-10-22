# Git Operations

The Git Operations tools provide programmatic access to git repository analysis, enabling automated workflows to understand what files have changed on a branch.

## Overview

Git tools help AI assistants and workflows understand the scope of changes in a repository by analyzing git history and working directory state. This enables intelligent operations like:

- Targeted code review of only changed files
- Selective testing based on modifications
- Impact analysis for pull requests
- Automated documentation updates for changed modules

## Key Concepts

### Branch Change Scope

The `git_changes` tool determines what files are relevant for a given branch by analyzing:

- **Feature/Issue Branches**: All files changed since the branch diverged from its parent, plus uncommitted changes
- **Main/Trunk Branches**: Only uncommitted changes (staged, unstaged, and untracked)

This distinction ensures workflows focus on the right scope:
- Feature branches show all work done on that branch
- Main branches only show pending changes not yet committed

### Parent Branch Detection

For branches following the `issue/*` pattern, the tool automatically detects the parent branch by analyzing git history. This enables workflows to work without explicit configuration about branch relationships.

## Available Tools

- [`git_changes`](changes.md) - List all files that have changed on a branch

## Use Cases

### Code Review Automation

```bash
# Get all changed files for review
sah git changes --branch issue/feature-123

# Returns: List of all files modified on this branch
```

### Selective Testing

Workflows can use git_changes to run tests only for affected modules:

```yaml
### test
Run tests for changed files
**Actions**:
  - tool: git_changes
    branch: $CURRENT_BRANCH
**Next**: report
```

### Documentation Generation

Generate documentation only for modules that have changed:

```bash
# Get changed Rust files
sah git changes --branch $BRANCH | grep '\.rs$'

# Generate docs for those modules
```

## Integration

Git tools integrate seamlessly with:

- **Workflows**: Use in action blocks to drive conditional logic
- **MCP Protocol**: Available as `git_changes` tool in Claude Code
- **CLI**: Direct command-line access via `sah git changes`

## Best Practices

### Branch Naming

Use consistent branch naming patterns (e.g., `issue/*`, `feature/*`) to enable automatic parent detection.

### Uncommitted Changes

Git tools include uncommitted changes in their analysis, ensuring workflows see the complete picture including:
- Staged files
- Unstaged modifications
- Untracked files

### Error Handling

Always handle cases where:
- The repository is not a git repository
- The branch does not exist
- Git operations fail due to repository corruption
