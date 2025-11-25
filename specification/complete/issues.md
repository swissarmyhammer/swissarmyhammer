# OBSOLETE: Issue System

**Status**: This specification is obsolete. The issue system has been removed.

**Replacement**: Use the rules + todos system:
- Issues → Todos (ephemeral tasks via `mcp__sah__todo_*` tools)
- Issue tracking → Rules (permanent acceptance criteria via `mcp__sah__rules_*` tools)

---

# Original Specification

_The content below describes the removed issue system for historical reference._

## Overview

Track issues directly in the repository, storing them in as files in git, with an MCP server.

## Rules

Issues are individual markdown files stored in the root of a repository in `./issues`.

Each Issue describes work to be done in markdown.

When issues are completed, they are moved to `./issues/complete`.

Each issue file name starts with a 6 digit increasing integer, like `<nnnnnn>_<more naming>.md`.

The file name before the .md is used as the issue name like `<issue_name>.md`

When a new issue is created, it need to start with the next highest number so that issues are sequential.

Issues are always worked on a work branch named `issue/<issue_name>`

## Use Cases

This will be an MCP Tool capability to swissarmyhammer.
Think deeply about creating great descriptions of the tool capability to allow Claude to call it reliably.
Research best practices for making MCP tools.

Swissarmyhammer now has multiple MCP tools, so calling them just 'mcp' is a bad idea.

### Create

As an LLM, I want to be able to create an issue using an MCP tool.

As an LLM, when a user reports an issue, I want to record it as an issue for later.

### Mark Complete

As an LLM, I want to be able to mark an issue complete using an MCP tool.

### All Complete

As an LLM, I want to be ask a YES/NO question if all issues are complete.

### Update

As an LLM, I want to be able to record additional context in an issue using an MCP tool.

### Current

As an LLM, I want to be able to ask for the current issue with an MCP tool

As an LLM, I want to know the current issue for the active work branch.

As an LLM, I want to know the current issue for the main branch.

### Work Issue

As an LLM, I want to switch to a work branch for an issue using git.

### Merge Issue

As an LLM, I want to merge my issue from a work branch  to the main branch.

