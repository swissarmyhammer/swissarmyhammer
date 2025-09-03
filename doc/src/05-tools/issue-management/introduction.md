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
- Issues can be linked to work branches using `issue/<issue_name>` pattern
- Automatic branch creation and switching
- Merge capabilities to integrate completed work

## Available Tools

- `issue_create` - Create new issues
- `issue_work` - Switch to work branch for an issue
- `issue_mark_complete` - Mark issues as complete
- `issue_merge` - Merge issue branches
- `issue_update` - Update issue content
- `issue_all_complete` - Check completion status
- `issue_list` - List all issues
- `issue_show` - Display issue details