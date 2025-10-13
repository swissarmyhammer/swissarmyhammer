# Issue Management

SwissArmyHammer provides a comprehensive issue tracking system that integrates with Git to manage work items as markdown files in your repository.

## Overview

Issues are stored as markdown files in the `./issues` directory and can be:
- Created and managed through MCP tools
- Linked to Git branches for workflow integration
- Marked as complete and moved to `./issues/complete`
- Tracked across project lifecycle

## Core Concepts

### Issue Structure
- Issues are markdown files with descriptive names
- Support both named (`feature_name.md`) and nameless (ULID-based) issues
- Contain full context and requirements in markdown format

### Git Integration
- Issues can be worked on from any branch
- Use your preferred git workflow for branching and merging
- The system tracks which issue you're currently working on

## Working with Issues

Issues can be worked on from any branch. The system tracks the "current issue" using a marker file (`.swissarmyhammer/.current_issue`) which allows you to:
- Work on issues from any git branch
- Switch between issues without branch management
- Use your preferred git workflow

Use `issue_show current` to see which issue you're currently working on.

## Available Tools

- `issue_create` - Create new issues
- `issue_mark_complete` - Mark issues as complete
- `issue_update` - Update issue content
- `issue_all_complete` - Check completion status
- `issue_list` - List all issues
- `issue_show` - Display issue details