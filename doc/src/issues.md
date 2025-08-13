# Issue Management

SwissArmyHammer provides a powerful issue tracking system that integrates directly with your Git workflow. Issues are stored as Markdown files in your repository, creating a self-contained, version-controlled task management system.

## Overview

The issue management system allows you to:
- Create and track work items as Markdown files
- Automatically generate unique issue identifiers
- Create feature branches for issue work
- Merge completed issues back to their source branch
- Track issue lifecycle and status
- Search and organize issues efficiently

## Core Concepts

### Issue Structure

Issues are stored as Markdown files in the `./issues/` directory with the following structure:

```
project/
├── issues/
│   ├── FEATURE_001_user-authentication.md
│   ├── BUG_002_login-validation.md
│   └── complete/
│       └── REFACTOR_003_code-cleanup.md
```

### Issue Naming

Issues follow a structured naming convention:
- `TYPE_NUMBER_description` (e.g., `FEATURE_001_user-auth`)
- Or auto-generated ULID for unnamed issues: `01K0Q4V1N0V35TQEDPXPE1HF7Z.md`

Supported issue types:
- `FEATURE` - New functionality
- `BUG` - Bug fixes
- `REFACTOR` - Code improvements
- `DOCS` - Documentation updates
- Custom types as needed

## Basic Usage

### Creating Issues

Create a new issue with a name:
```bash
sah issue create --name "feature_user_auth" --content "# User Authentication

Implement user login and registration system.

## Requirements
- Email/password login
- Session management
- Password reset functionality
"
```

Create a quick unnamed issue:
```bash
echo "# Quick Bug Fix
Fix the validation error in login form" | sah issue create
```

Create from file:
```bash
sah issue create --file issue_template.md
```

### Listing Issues

List all active issues:
```bash
sah issue list
```

Include completed issues:
```bash
sah issue list --completed --active
```

Output in different formats:
```bash
sah issue list --format json
sah issue list --format table
```

### Viewing Issues

Show a specific issue:
```bash
sah issue show FEATURE_001_user-auth
```

Show the current issue (based on branch):
```bash
sah issue show current
```

Show the next pending issue:
```bash
sah issue show next
```

Raw content only:
```bash
sah issue show FEATURE_001_user-auth --raw
```

## Git Integration

### Working on Issues

Start work on an issue (creates/switches to branch):
```bash
sah issue work FEATURE_001_user-auth
```

This creates and switches to a branch named `issue/FEATURE_001_user-auth`.

### Current Issue Status

Check which issue you're currently working on:
```bash
sah issue current
```

Show overall status:
```bash
sah issue status
```

### Completing Issues

Mark an issue as complete:
```bash
sah issue complete FEATURE_001_user-auth
```

This moves the issue file to `./issues/complete/`.

### Merging Issue Work

Merge completed issue work back to source branch:
```bash
sah issue merge FEATURE_001_user-auth
```

Delete the branch after merging:
```bash
sah issue merge FEATURE_001_user-auth --delete-branch
```

## Advanced Features

### Updating Issues

Add content to existing issues:
```bash
sah issue update FEATURE_001_user-auth --content "
## Additional Context
Added OAuth integration requirements.
" --append
```

Replace entire content:
```bash
sah issue update FEATURE_001_user-auth --file updated_requirements.md
```

### Issue Templates

Create template files for consistent issue creation:

```markdown
# {ISSUE_TYPE}: {TITLE}

## Description
Brief description of the issue.

## Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Technical Notes
Implementation details and considerations.

## Testing
Testing approach and requirements.
```

### Branch Management Strategies

**Feature Branches**: Each issue gets its own branch
```bash
sah issue work FEATURE_001_user-auth
# Work on feature
git commit -m "implement user authentication"
sah issue complete FEATURE_001_user-auth
sah issue merge FEATURE_001_user-auth --delete-branch
```

**Long-running Features**: Keep branches for complex features
```bash
sah issue work FEATURE_001_user-auth
# Multiple commits over time
git commit -m "add login form"
git commit -m "add validation"
git commit -m "add tests"
sah issue merge FEATURE_001_user-auth  # Keep branch for future work
```

## Organization and Search

### Issue Organization

Organize issues using:
- **Directory structure**: Group related issues in subdirectories
- **Naming conventions**: Use consistent prefixes and descriptions
- **Tags**: Add tags within issue content for categorization
- **Labels**: Use Markdown headers and lists for status tracking

### Searching Issues

Search issue content:
```bash
sah search query "authentication login"
```

Use grep for specific patterns:
```bash
grep -r "TODO" issues/
```

List issues by type:
```bash
ls issues/FEATURE_*
ls issues/BUG_*
```

## Best Practices

### Issue Creation
- Use descriptive names that clearly identify the work
- Include clear acceptance criteria
- Add relevant context and background
- Link to related issues or documentation

### Content Structure
```markdown
# Issue Title

## Summary
Brief overview of what needs to be done.

## Details
Detailed requirements and specifications.

## Acceptance Criteria
- [ ] Specific, measurable criteria
- [ ] Each criterion should be testable
- [ ] Include both functional and non-functional requirements

## Technical Notes
Implementation approach, architecture decisions, dependencies.

## Resources
- Links to relevant documentation
- Related issues or PRs
- Design mockups or specifications
```

### Workflow Integration
- Create issues before starting work
- Use descriptive commit messages that reference issues
- Review and update issues as work progresses
- Mark issues complete only when fully tested
- Clean up branches regularly

### Team Collaboration
- Use consistent naming conventions
- Include team members in issue discussions
- Document decisions and changes in issue comments
- Use issue references in commit messages and PRs

## Integration with Workflows

Issues integrate seamlessly with SwissArmyHammer workflows:

```markdown
# example_workflow.md
## Workflow: Issue Development

1. Create issue: `sah issue create --name "feature_name"`
2. Start work: `sah issue work {issue_name}`
3. Development cycle:
   - Code changes
   - Commit with issue reference
   - Update issue with progress
4. Complete: `sah issue complete {issue_name}`
5. Merge: `sah issue merge {issue_name} --delete-branch`
```

## Troubleshooting

### Common Issues

**Issue not found**:
- Check issue name spelling
- Verify issue exists: `sah issue list`
- Check if issue was completed: `sah issue list --completed`

**Branch conflicts**:
- Ensure working directory is clean before switching branches
- Resolve merge conflicts before completing issues
- Use `git status` to check current state

**Git integration problems**:
- Verify Git repository is initialized
- Check branch permissions and remote access
- Ensure working directory is within a Git repository

### Error Messages

**"Issue already exists"**: Issue name conflicts with existing issue
**"Branch already exists"**: Git branch name conflicts - use different issue name or clean up branches
**"No current issue"**: Not currently on an issue branch - use `sah issue current` to check status

## Migration and Maintenance

### Migrating from Other Systems

Convert from linear issue numbers:
```bash
# Convert JIRA-style issues
for issue in PROJ-*.md; do
  mv "$issue" "FEATURE_$(basename $issue .md | sed 's/PROJ-//')_$(head -1 $issue | tr ' ' '-').md"
done
```

### Cleanup and Maintenance

Regularly clean up completed issues:
```bash
# Archive old completed issues
mkdir -p issues/archive/$(date +%Y)
mv issues/complete/*.md issues/archive/$(date +%Y)/
```

Remove stale branches:
```bash
# List issue branches
git branch | grep "issue/"

# Clean up merged branches
git branch -d issue/FEATURE_001_user-auth
```

## API Reference

For programmatic access to issue management, see the [Rust API documentation](rust-api.md#issue-management).

Key types and functions:
- `IssueName` - Type-safe issue name handling
- `IssueStorage` - Issue persistence interface
- `create_issue()` - Create new issues programmatically
- `list_issues()` - Query and filter issues
- `issue_lifecycle()` - Manage issue state transitions