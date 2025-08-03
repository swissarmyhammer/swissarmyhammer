# Issue Management User Guide

This guide covers the complete workflow for managing issues in SwissArmyHammer.

## Overview

SwissArmyHammer's issue management system provides a lightweight, git-friendly way to track work items directly in your repository. Issues are stored as markdown files with sequential numbering and can be managed through both MCP tools and CLI commands.

## Table of Contents

- [Overview](#overview)
- [Directory Structure](#directory-structure)
- [Workflow Patterns](#workflow-patterns)
  - [Basic Workflow](#basic-workflow)
  - [Advanced Workflow](#advanced-workflow)
- [MCP Tools Reference](#mcp-tools-reference)
  - [issue_create](#issue_create)
  - [issue_work](#issue_work)
  - [issue_update](#issue_update)
  - [issue_mark_complete](#issue_mark_complete)
  - [issue_show](#issue_show)
  - [issue_all_complete](#issue_all_complete)
  - [issue_merge](#issue_merge)
- [CLI Commands Reference](#cli-commands-reference)
  - [Create Issues](#create-issues)
  - [List Issues](#list-issues)
  - [Show Issue Details](#show-issue-details)
  - [Update Issues](#update-issues)
  - [Work on Issues](#work-on-issues)
  - [Complete and Merge](#complete-and-merge)
- [Best Practices](#best-practices)
  - [Issue Naming](#issue-naming)
  - [Issue Content](#issue-content)
  - [Git Workflow](#git-workflow)
  - [Team Coordination](#team-coordination)
- [Troubleshooting](#troubleshooting)
  - [Git Repository Issues](#git-repository-issues)
  - [Issue Management Issues](#issue-management-issues)
  - [Performance Issues](#performance-issues)
- [Integration Examples](#integration-examples)
  - [With Claude Code](#with-claude-code)
  - [With GitHub Actions](#with-github-actions)
  - [With VS Code](#with-vs-code)
- [Advanced Usage](#advanced-usage)
  - [Batch Operations](#batch-operations)
  - [Custom Workflows](#custom-workflows)
  - [Issue Analytics](#issue-analytics)
- [Configuration](#configuration)
  - [Global Configuration](#global-configuration)
  - [Project Configuration](#project-configuration)
- [API Reference](#api-reference)
  - [Issue File Format](#issue-file-format)
  - [Exit Codes](#exit-codes)
  - [Environment Variables](#environment-variables)
- [Performance Optimization](#performance-optimization)
  - [Large Projects](#large-projects)
  - [Memory Usage](#memory-usage)
- [Migration Guide](#migration-guide)
  - [From GitHub Issues](#from-github-issues)
  - [From Jira](#from-jira)
- [Contributing](#contributing)
- [License](#license)

## Directory Structure

```
your-project/
├── issues/
│   ├── 000001_implement_auth.md      # Active issue
│   ├── 000002_fix_bug.md            # Active issue
│   └── complete/
│       └── 000003_add_tests.md      # Completed issue
├── .git/
└── your-code/
```

## Workflow Patterns

### Basic Workflow

1. **Create Issue**: Define what needs to be done
2. **Work on Issue**: Switch to dedicated branch
3. **Update Progress**: Document work as you go
4. **Mark Complete**: Signal work is finished
5. **Merge**: Integrate work into main branch

### Advanced Workflow

1. **Project Planning**: Create multiple issues for features
2. **Priority Management**: Work on issues in order
3. **Progress Tracking**: Regular updates and status checks
4. **Team Coordination**: Use issue status for team awareness
5. **Historical Record**: Completed issues provide project history

## MCP Tools Reference

### issue_create

Create a new issue with automatic ULID generation.

**Parameters:**
- `name` (optional): Issue name for filename - if omitted, uses auto-generated ULID
- `content` (required): Markdown content

**Example:**
```json
{
  "tool": "issue_create",
  "arguments": {
    "name": "implement_dashboard",
    "content": "# Dashboard Implementation\n\nCreate a user dashboard with the following features:\n- User profile display\n- Activity feed\n- Quick actions"
  }
}
```

**Example (auto-generated name):**
```json
{
  "tool": "issue_create",
  "arguments": {
    "content": "# Quick Bug Fix\n\nFix the navigation issue in the header."
  }
}
```

### issue_work

Start working on an issue by creating and switching to a work branch.

**Parameters:**
- `name` (required): Issue name (full filename without .md extension) to work on

**Example:**
```json
{
  "tool": "issue_work",
  "arguments": {
    "name": "implement_dashboard"
  }
}
```

### issue_update

Update an existing issue's content.

**Parameters:**
- `name` (required): Issue name to update
- `content` (required): New or additional content
- `append` (optional): If true, append to existing content (default: false)

**Example:**
```json
{
  "tool": "issue_update",
  "arguments": {
    "name": "implement_dashboard",
    "content": "## Progress Update\n\nCompleted user profile display component.",
    "append": true
  }
}
```

### issue_mark_complete

Mark an issue as complete, moving it to the completed directory.

**Parameters:**
- `name` (required): Issue name to complete

**Example:**
```json
{
  "tool": "issue_mark_complete",
  "arguments": {
    "name": "implement_dashboard"
  }
}
```

### issue_show

Display details of a specific issue by name, or use special parameters to get current/next issues.

**Parameters:**
- `name` (required): Name of the issue to show, or special values:
  - `"current"`: Show the issue for the current git branch
  - `"next"`: Show the next pending issue alphabetically
- `raw` (optional): Show raw content only without formatting (default: false)

**Examples:**
```json
{
  "tool": "issue_show",
  "arguments": {
    "name": "FEATURE_000123_user-auth"
  }
}
```

```json
{
  "tool": "issue_show",
  "arguments": {
    "name": "current"
  }
}
```

```json
{
  "tool": "issue_show",
  "arguments": {
    "name": "next"
  }
}
```

### issue_all_complete

Check if all issues in the project are completed.

**Parameters:** None

**Example:**
```json
{
  "tool": "issue_all_complete",
  "arguments": {}
}
```

### issue_merge

Merge completed issue work back to the main branch.

**Parameters:**
- `name` (required): Issue name to merge
- `delete_branch` (optional): Whether to delete the branch after merging (default: false)

**Example:**
```json
{
  "tool": "issue_merge",
  "arguments": {
    "name": "implement_dashboard"
  }
}
```

**Example (with branch deletion):**
```json
{
  "tool": "issue_merge",
  "arguments": {
    "name": "implement_dashboard",
    "delete_branch": true
  }
}
```

### issue_list

List all available issues with their status and metadata.

**Parameters:**
- `show_completed` (optional): Include completed issues in the list (default: false)
- `show_active` (optional): Include active issues in the list (default: true)  
- `format` (optional): Output format - "table", "json", or "markdown" (default: "table")

**Example:**
```json
{
  "tool": "issue_list",
  "arguments": {
    "show_completed": true,
    "format": "json"
  }
}
```

## CLI Commands Reference

### Create Issues

```bash
# Create with inline content
swissarmyhammer issue create "fix_login_bug" --content "Fix the login validation issue"

# Create with content from file
swissarmyhammer issue create "add_feature" --file feature_spec.md

# Create with content from stdin
echo "Issue content" | swissarmyhammer issue create "my_issue" --content -
```

### List Issues

```bash
# List all issues
swissarmyhammer issue list

# List only active issues
swissarmyhammer issue list --active

# List only completed issues
swissarmyhammer issue list --completed

# Output as JSON
swissarmyhammer issue list --format json

# Output as Markdown
swissarmyhammer issue list --format markdown
```

### Show Issue Details

```bash
# Show formatted issue details
swissarmyhammer issue show 1

# Show raw content only
swissarmyhammer issue show 1 --raw
```

### Update Issues

```bash
# Replace content
swissarmyhammer issue update 1 --content "New content"

# Append to existing content
swissarmyhammer issue update 1 --content "Additional notes" --append

# Update from file
swissarmyhammer issue update 1 --file update.md --append
```

### Work on Issues

```bash
# Start working on an issue
swissarmyhammer issue work 1

# Check current issue
swissarmyhammer issue current

# Show project status
swissarmyhammer issue status
```

### Complete and Merge

```bash
# Mark issue complete
swissarmyhammer issue complete 1

# Merge to main branch
swissarmyhammer issue merge 1

# Merge and keep branch
swissarmyhammer issue merge 1 --keep-branch
```

## Best Practices

### Issue Naming

- Use descriptive, action-oriented names
- Use underscores instead of spaces
- Keep names under 50 characters
- Examples: `implement_auth`, `fix_login_bug`, `add_user_dashboard`

### Issue Content

- Use markdown for formatting
- Include acceptance criteria
- Add relevant links and references
- Update with progress notes
- Keep content focused and organized

### Git Workflow

- Always work on issue branches
- Make atomic commits with clear messages
- Keep branches up to date with main
- Complete issues before merging
- Use descriptive commit messages

### Team Coordination

- Check issue status before starting work
- Update issues with progress regularly
- Use issue comments for team communication
- Mark issues complete when work is done
- Review completed issues in team meetings

## Troubleshooting

### Git Repository Issues

**Problem**: "Not in a git repository"
**Solution**: Initialize git repository in your project directory:
```bash
git init
git add .
git commit -m "Initial commit"
```

**Problem**: "Uncommitted changes prevent operation"
**Solution**: Commit or stash your changes:
```bash
git add .
git commit -m "Work in progress"
# or
git stash
```

### Issue Management Issues

**Problem**: "Issue not found"
**Solution**: Check available issues:
```bash
swissarmyhammer issue list
```

**Problem**: "Branch already exists"
**Solution**: Switch to existing branch or delete it:
```bash
git checkout issue/000001_issue_name
# or
git branch -D issue/000001_issue_name
```

### Performance Issues

**Problem**: Slow issue operations with many issues
**Solution**: Use filtering and pagination:
```bash
swissarmyhammer issue list --active
swissarmyhammer issue list --format json | jq '.[] | select(.completed == false)'
```

## Integration Examples

### With Claude Code

```
Human: Create an issue to implement user authentication
Assistant: I'll create an issue to track implementing user authentication.

issue_create name="implement_auth" content="# User Authentication Implementation

## Requirements
- JWT-based authentication
- Login/logout endpoints
- Password hashing with bcrypt
- Session management
- Role-based access control

## Acceptance Criteria
- [ ] User registration endpoint
- [ ] Login endpoint with JWT token generation
- [ ] Password validation and hashing
- [ ] Protected routes middleware
- [ ] Role-based permissions
- [ ] Session expiration handling
- [ ] Unit tests for all auth components
"
```

### With GitHub Actions

```yaml
name: Issue Management Workflow

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  check-issues:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v2
    
    - name: Setup SwissArmyHammer
      run: |
        # Install SwissArmyHammer
        curl -sSL https://install.swissarmyhammer.dev | bash
        
    - name: Check issue status
      run: |
        swissarmyhammer issue status
        
    - name: Validate issue files
      run: |
        swissarmyhammer validate --path issues/
```

### With VS Code

Create a `.vscode/tasks.json` file:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Create Issue",
      "type": "shell",
      "command": "swissarmyhammer",
      "args": ["issue", "create", "${input:issueName}", "--content", "${input:issueContent}"],
      "group": "build",
      "presentation": {
        "echo": true,
        "reveal": "always",
        "focus": false,
        "panel": "shared"
      }
    },
    {
      "label": "List Issues",
      "type": "shell",
      "command": "swissarmyhammer",
      "args": ["issue", "list"],
      "group": "build"
    },
    {
      "label": "Current Issue",
      "type": "shell",
      "command": "swissarmyhammer",
      "args": ["issue", "current"],
      "group": "build"
    }
  ],
  "inputs": [
    {
      "id": "issueName",
      "description": "Issue name",
      "default": "new_issue",
      "type": "promptString"
    },
    {
      "id": "issueContent",
      "description": "Issue content",
      "default": "# New Issue\n\nDescribe the issue here.",
      "type": "promptString"
    }
  ]
}
```

## Advanced Usage

### Batch Operations

```bash
# Create multiple issues from a CSV file
cat issues.csv | while IFS=',' read -r name content; do
  swissarmyhammer issue create "$name" --content "$content"
done

# Update all active issues with a common note
swissarmyhammer issue list --active --format json | \
  jq -r '.[] | .number' | \
  while read -r number; do
    swissarmyhammer issue update "$number" --content "\n\n**Updated:** $(date)" --append
  done
```

### Custom Workflows

```bash
#!/bin/bash
# smart-issue-create.sh - Smart issue creation with templates

ISSUE_NAME="$1"
ISSUE_TYPE="$2"

case "$ISSUE_TYPE" in
  "feature")
    TEMPLATE="# Feature: $ISSUE_NAME

## Overview
[Brief description of the feature]

## Requirements
- [ ] Requirement 1
- [ ] Requirement 2

## Acceptance Criteria
- [ ] Criteria 1
- [ ] Criteria 2

## Implementation Notes
[Technical notes and considerations]
"
    ;;
  "bug")
    TEMPLATE="# Bug Fix: $ISSUE_NAME

## Problem
[Describe the bug]

## Steps to Reproduce
1. Step 1
2. Step 2
3. Step 3

## Expected Behavior
[What should happen]

## Actual Behavior
[What actually happens]

## Fix Strategy
[How to fix it]
"
    ;;
  *)
    TEMPLATE="# $ISSUE_NAME

[Issue description]
"
    ;;
esac

swissarmyhammer issue create "$ISSUE_NAME" --content "$TEMPLATE"
```

### Issue Analytics

```bash
#!/bin/bash
# issue-analytics.sh - Generate issue statistics

echo "=== Issue Analytics ==="
echo ""

# Count active issues
ACTIVE_COUNT=$(swissarmyhammer issue list --active --format json | jq length)
echo "Active Issues: $ACTIVE_COUNT"

# Count completed issues
COMPLETED_COUNT=$(swissarmyhammer issue list --completed --format json | jq length)
echo "Completed Issues: $COMPLETED_COUNT"

# Calculate completion rate
TOTAL_COUNT=$((ACTIVE_COUNT + COMPLETED_COUNT))
if [ $TOTAL_COUNT -gt 0 ]; then
  COMPLETION_RATE=$((COMPLETED_COUNT * 100 / TOTAL_COUNT))
  echo "Completion Rate: $COMPLETION_RATE%"
fi

# Show recent activity
echo ""
echo "=== Recent Activity ==="
swissarmyhammer issue list --completed --format json | \
  jq -r '.[] | select(.completed_at != null) | "\(.number): \(.name) (completed: \(.completed_at))"' | \
  sort -k3 -r | head -5
```

## Configuration

### Global Configuration

Create `~/.swissarmyhammer/config.toml`:

```toml
[issues]
# Default issue directory
directory = "./issues"

# Default branch prefix for issue work
branch_prefix = "issue"

# Auto-delete branches after merge
auto_delete_branches = true

# Default issue template
template = """
# {name}

## Overview
[Brief description]

## Tasks
- [ ] Task 1
- [ ] Task 2

## Notes
[Additional notes]
"""

[git]
# Default commit message template
commit_template = "{action}: {issue_name} - {description}"

# Auto-commit issue updates
auto_commit = false
```

### Project Configuration

Create `.swissarmyhammer/config.toml` in your project:

```toml
[issues]
# Project-specific issue directory
directory = "./project-issues"

# Custom issue number format
number_format = "PRJ-{:06d}"

# Issue categories
categories = ["bug", "feature", "enhancement", "documentation"]

# Required fields
required_fields = ["assignee", "priority", "category"]

[workflow]
# Required before marking complete
completion_checklist = [
  "Code written and tested",
  "Documentation updated",
  "Tests passing",
  "Code reviewed"
]
```

## API Reference

### Issue File Format

Issues are stored as markdown files with optional YAML frontmatter:

```markdown
---
title: "Fix login bug"
assignee: "john@example.com"
priority: "high"
category: "bug"
created_at: "2024-01-15T10:30:00Z"
updated_at: "2024-01-15T14:45:00Z"
---

# Fix Login Bug

## Problem
Users cannot log in with special characters in their passwords.

## Root Cause
Password validation is too strict and rejects valid special characters.

## Solution
Update the password validation regex to allow all printable ASCII characters.

## Implementation
- [ ] Update validation regex in `auth.rs:45`
- [ ] Add unit tests for special character passwords
- [ ] Update documentation

## Testing
- [ ] Test with various special characters
- [ ] Test with existing passwords
- [ ] Test edge cases
```

### Exit Codes

| Code | Description |
|------|-------------|
| 0    | Success |
| 1    | General error |
| 2    | Invalid arguments |
| 3    | File not found |
| 4    | Git error |
| 5    | Permission denied |
| 10   | Issue not found |
| 11   | Issue already exists |
| 12   | Invalid issue state |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SWISSARMYHAMMER_ISSUES_DIR` | Issue directory | `./issues` |
| `SWISSARMYHAMMER_BRANCH_PREFIX` | Branch prefix | `issue` |
| `SWISSARMYHAMMER_AUTO_DELETE_BRANCHES` | Auto-delete branches | `true` |
| `SWISSARMYHAMMER_EDITOR` | Default editor | `$EDITOR` |

### Rust API Types

For programmatic access to issue management functionality, the following types are available:

#### Core Types

- [`Issue`](./api/swissarmyhammer/issues/struct.Issue.html) - Represents a single issue with metadata
- [`IssueState`](./api/swissarmyhammer/issues/enum.IssueState.html) - Issue lifecycle state (Active, Complete, etc.)
- [`IssueStorage`](./api/swissarmyhammer/issues/trait.IssueStorage.html) - Storage abstraction for issues

#### Storage Implementations

- [`FileSystemIssueStorage`](./api/swissarmyhammer/issues/struct.FileSystemIssueStorage.html) - File-based storage backend
- [`InstrumentedIssueStorage`](./api/swissarmyhammer/issues/struct.InstrumentedIssueStorage.html) - Instrumented wrapper with metrics

#### Utility Types

- [`ProjectStatus`](./api/swissarmyhammer/issues/struct.ProjectStatus.html) - Project-wide issue status information
- [`IssueBranchResult`](./api/swissarmyhammer/issues/struct.IssueBranchResult.html) - Result of branch operations
- [`IssueMergeResult`](./api/swissarmyhammer/issues/struct.IssueMergeResult.html) - Result of merge operations
- [`PerformanceMetrics`](./api/swissarmyhammer/issues/struct.PerformanceMetrics.html) - Performance monitoring data

#### Utility Functions

- [`get_current_issue_from_branch`](./api/swissarmyhammer/issues/fn.get_current_issue_from_branch.html) - Extract issue from branch name
- [`work_on_issue`](./api/swissarmyhammer/issues/fn.work_on_issue.html) - Switch to issue work branch
- [`merge_issue_branch`](./api/swissarmyhammer/issues/fn.merge_issue_branch.html) - Merge issue work back to main
- [`get_next_issue`](./api/swissarmyhammer/issues/fn.get_next_issue.html) - Find next issue to work on

**See Also:**
- [API Guide](./api-guide.md) - Comprehensive API usage guide
- [Library Examples](./library-examples.md) - Practical code examples
- [Rustdoc API Documentation](./api/swissarmyhammer/index.html) - Complete API reference

## Performance Optimization

### Large Projects

For projects with many issues, consider:

1. **Use filtering**: Always use `--active` or `--completed` flags
2. **JSON output**: Use `--format json` for programmatic access
3. **Batch operations**: Group multiple operations together
4. **Index optimization**: Keep issue files small and focused

### Memory Usage

```bash
# Monitor memory usage during large operations
time swissarmyhammer issue list --format json > /dev/null

# Use streaming for large datasets
swissarmyhammer issue list --format json | jq -c '.[]' | while read -r issue; do
  # Process each issue individually
  echo "$issue" | jq -r '.name'
done
```

## Migration Guide

### From GitHub Issues

```bash
#!/bin/bash
# migrate-from-github.sh - Migrate GitHub issues to SwissArmyHammer

REPO="owner/repo"
TOKEN="your-github-token"

# Export GitHub issues
gh issue list --repo "$REPO" --state all --json number,title,body,state | \
  jq -r '.[] | @base64' | \
  while read -r issue; do
    # Decode issue data
    ISSUE_DATA=$(echo "$issue" | base64 -d)
    NUMBER=$(echo "$ISSUE_DATA" | jq -r '.number')
    TITLE=$(echo "$ISSUE_DATA" | jq -r '.title')
    BODY=$(echo "$ISSUE_DATA" | jq -r '.body')
    STATE=$(echo "$ISSUE_DATA" | jq -r '.state')
    
    # Create SwissArmyHammer issue
    ISSUE_NAME=$(echo "$TITLE" | sed 's/[^a-zA-Z0-9]/_/g' | tr '[:upper:]' '[:lower:]')
    CONTENT="# $TITLE

$BODY

---
*Migrated from GitHub Issue #$NUMBER*
"
    
    swissarmyhammer issue create "$ISSUE_NAME" --content "$CONTENT"
    
    # Mark completed issues as complete
    if [ "$STATE" = "closed" ]; then
      ISSUE_NUM=$(swissarmyhammer issue list --format json | jq -r '.[] | select(.name == "'$ISSUE_NAME'") | .number')
      swissarmyhammer issue complete "$ISSUE_NUM"
    fi
  done
```

### From Jira

```bash
#!/bin/bash
# migrate-from-jira.sh - Migrate Jira issues to SwissArmyHammer

JIRA_URL="https://your-domain.atlassian.net"
PROJECT="YOUR-PROJECT"
EMAIL="your-email@example.com"
TOKEN="your-jira-token"

# Export Jira issues
curl -u "$EMAIL:$TOKEN" \
  "$JIRA_URL/rest/api/2/search?jql=project=$PROJECT&maxResults=1000" | \
  jq -r '.issues[] | @base64' | \
  while read -r issue; do
    # Decode and process issue data
    ISSUE_DATA=$(echo "$issue" | base64 -d)
    KEY=$(echo "$ISSUE_DATA" | jq -r '.key')
    SUMMARY=$(echo "$ISSUE_DATA" | jq -r '.fields.summary')
    DESCRIPTION=$(echo "$ISSUE_DATA" | jq -r '.fields.description // ""')
    STATUS=$(echo "$ISSUE_DATA" | jq -r '.fields.status.name')
    
    # Create SwissArmyHammer issue
    ISSUE_NAME=$(echo "$SUMMARY" | sed 's/[^a-zA-Z0-9]/_/g' | tr '[:upper:]' '[:lower:]')
    CONTENT="# $SUMMARY

$DESCRIPTION

---
*Migrated from Jira Issue $KEY*
"
    
    swissarmyhammer issue create "$ISSUE_NAME" --content "$CONTENT"
    
    # Mark completed issues as complete
    if [ "$STATUS" = "Done" ] || [ "$STATUS" = "Resolved" ]; then
      ISSUE_NUM=$(swissarmyhammer issue list --format json | jq -r '.[] | select(.name == "'$ISSUE_NAME'") | .number')
      swissarmyhammer issue complete "$ISSUE_NUM"
    fi
  done
```

## Contributing

To contribute to SwissArmyHammer's issue management system:

1. **Report Issues**: Use SwissArmyHammer itself to track bugs and features
2. **Submit PRs**: Follow the standard GitHub workflow
3. **Write Tests**: Ensure all new features have comprehensive tests
4. **Update Documentation**: Keep this guide up to date

### Development Setup

```bash
# Clone the repository
git clone https://github.com/swissarmyhammer/swissarmyhammer.git
cd swissarmyhammer

# Install dependencies
cargo build

# Run tests
cargo test

# Create a development issue
./target/debug/swissarmyhammer issue create "my_feature" --content "# My Feature

Description of what I want to implement.
"

# Start working on it
./target/debug/swissarmyhammer issue work 1
```

## License

This issue management system is part of SwissArmyHammer and is licensed under the MIT License. See [LICENSE](LICENSE) for details.