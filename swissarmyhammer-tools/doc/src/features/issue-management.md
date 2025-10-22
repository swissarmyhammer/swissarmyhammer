# Issue Management

SwissArmyHammer provides comprehensive issue tracking stored as markdown files with complete lifecycle support and git-friendly storage.

## Overview

Issues are work items stored as markdown files in the `.swissarmyhammer/issues/` directory. This approach provides:

- **Human Readable**: Plain text files are easy to read and edit
- **Git Friendly**: Markdown diffs well in version control
- **Portable**: No proprietary formats or databases
- **Searchable**: Standard text search tools work out of the box

## Issue Lifecycle

```
┌─────────┐
│ Create  │
└────┬────┘
     │
     ▼
┌─────────┐     ┌────────┐
│ Active  │────>│ Update │
└────┬────┘     └────────┘
     │
     ▼
┌─────────┐
│Complete │
└─────────┘
```

## Available Tools

### issue_create

Create a new issue stored as a markdown file.

**Parameters:**
- `content` (required): Markdown content of the issue
- `name` (optional): Name of the issue (used in filename). When omitted, a ULID is auto-generated

**Example:**
```json
{
  "name": "add-user-authentication",
  "content": "# Add User Authentication\n\nImplement JWT-based authentication for the API.\n\n## Requirements\n\n- Login endpoint\n- Token validation middleware\n- User session management"
}
```

**Features:**
- Auto-generates ULID if name not provided
- Creates `.swissarmyhammer/issues/` directory if needed
- Validates markdown content
- Returns confirmation with issue name

### issue_list

List all available issues with their status and metadata.

**Parameters:**
- `show_completed` (optional): Include completed issues (default: false)
- `show_active` (optional): Include active issues (default: true)
- `format` (optional): Output format - `table`, `json`, or `markdown` (default: `table`)

**Example:**
```json
{
  "show_completed": false,
  "format": "table"
}
```

**Features:**
- Filter by status (active/completed)
- Multiple output formats
- Shows creation dates
- Displays file paths

### issue_show

Display details of a specific issue by name.

**Parameters:**
- `name` (required): Name of the issue to show. Use `next` for the next pending issue.
- `raw` (optional): Show raw content only without formatting (default: false)

**Example:**
```json
{
  "name": "add-user-authentication"
}
```

**Features:**
- Display full issue content
- Show metadata (status, creation date)
- Raw mode for processing
- Special `next` keyword for workflow automation

### issue_update

Update the content of an existing issue.

**Parameters:**
- `name` (required): Issue name to update
- `content` (required): New markdown content
- `append` (optional): Append to existing content instead of replacing (default: false)

**Example:**
```json
{
  "name": "add-user-authentication",
  "content": "## Progress\n\n- [x] Login endpoint implemented\n- [ ] Token validation pending",
  "append": true
}
```

**Features:**
- Replace or append content
- Preserves file metadata
- Validates markdown
- Atomic updates

### issue_mark_complete

Mark an issue as complete by moving it to the completed directory.

**Parameters:**
- `name` (required): Issue name to mark as complete

**Example:**
```json
{
  "name": "add-user-authentication"
}
```

**Features:**
- Moves file to `.swissarmyhammer/issues/complete/`
- Preserves issue content
- Updates metadata
- Returns confirmation

### issue_all_complete

Check if all issues are completed.

**Parameters:** None

**Example:**
```json
{}
```

**Returns:**
- Boolean indicating completion status
- Summary of pending and completed issues

## Issue Naming Conventions

### Automatic Naming

When no name is provided, issues are named with a ULID:
```
01K86037G1R1V0WGBDX4QKBS6M.md
```

### Manual Naming

Use descriptive names with prefixes:
```
FEATURE_add-user-authentication.md
BUG_fix-login-redirect.md
REFACTOR_cleanup-api-handlers.md
DOCS_update-readme.md
```

## Storage Structure

```
.swissarmyhammer/
└── issues/
    ├── active-issue-1.md
    ├── active-issue-2.md
    └── complete/
        ├── completed-issue-1.md
        └── completed-issue-2.md
```

## Best Practices

### Creating Issues

1. **Use Descriptive Names**: Choose names that clearly indicate the work
2. **Write Clear Content**: Use markdown formatting for readability
3. **Include Context**: Add requirements, constraints, and acceptance criteria
4. **Use Checklists**: Track progress with markdown task lists

### Organizing Issues

1. **Prefix by Type**: Use FEATURE_, BUG_, REFACTOR_, etc.
2. **Keep Active Small**: Complete issues when done to reduce clutter
3. **Archive Regularly**: Completed issues are automatically archived

### Updating Issues

1. **Append Progress**: Use `append: true` to add progress notes
2. **Replace for Rewrites**: Use `append: false` for major updates
3. **Track Decisions**: Document decisions and rationale in issue

### Completing Issues

1. **Complete When Done**: Mark issues complete as soon as work is finished
2. **Review Before Completing**: Ensure all requirements are met
3. **Keep History**: Completed issues remain accessible in `complete/`

## Markdown Format

Issues support full markdown:

```markdown
# Issue Title

Brief description of the issue.

## Background

Context and motivation for this work.

## Requirements

- [ ] Requirement 1
- [ ] Requirement 2
- [ ] Requirement 3

## Implementation Notes

Technical details and approach.

## Testing

How to verify the implementation.

## Related Issues

- See also: other-issue-name
```

## Workflow Integration

### Next Issue Pattern

```json
{
  "name": "next"
}
```

This returns the next active issue, enabling workflow automation.

### Batch Operations

List all issues, then process each:

```json
{
  "show_active": true,
  "format": "json"
}
```

### Progress Tracking

Update issues with progress:

```json
{
  "name": "issue-name",
  "content": "\n\n## Progress Update\n\nCompleted X, working on Y.",
  "append": true
}
```

## Git Integration

### Committing Issues

Issues should be committed to version control:

```bash
git add .swissarmyhammer/issues/
git commit -m "Add user authentication issue"
```

### Reviewing Changes

Issue changes are easy to review:

```bash
git diff .swissarmyhammer/issues/
```

### Collaborative Work

Multiple developers can work with issues:
- Each developer sees the same issues
- Changes merge cleanly (text files)
- Conflicts are rare and easy to resolve

## Common Use Cases

### Creating a Feature Issue

```json
{
  "name": "FEATURE_add-dark-mode",
  "content": "# Add Dark Mode\n\nImplement dark mode theme toggle.\n\n## Requirements\n\n- [ ] Theme switcher component\n- [ ] Dark color palette\n- [ ] Persist user preference"
}
```

### Tracking Bug Fixes

```json
{
  "name": "BUG_fix-memory-leak",
  "content": "# Fix Memory Leak in File Watcher\n\n## Issue\n\nFile watcher not releasing handles.\n\n## Root Cause\n\nMissing drop implementation.\n\n## Solution\n\nImplement proper cleanup."
}
```

### Documentation Tasks

```json
{
  "name": "DOCS_api-reference",
  "content": "# Create API Reference Documentation\n\n## Scope\n\n- Document all public APIs\n- Add usage examples\n- Include error handling"
}
```

## Limitations

- **No Dependencies**: Issues don't track dependencies between each other
- **No Assignments**: No built-in assignee tracking
- **No Priorities**: No priority levels (use prefixes or tags in content)
- **No Time Tracking**: No built-in time tracking

Use issue content (markdown) to add these features if needed.

## Next Steps

- [Workflow Execution](./workflow-execution.md): Automate issue processing
- [File Operations](./file-operations.md): Read and write issue files
- [Git Integration](./git-integration.md): Track changes with git
