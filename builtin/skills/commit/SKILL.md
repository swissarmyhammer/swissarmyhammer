---
name: commit
description: Create a well-structured git commit with a conventional commit message. Use when the user wants to commit their current changes.
allowed-tools: "*"
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Commit

Create a git commit with a well-crafted conventional commit message.

## How to Execute

Use the `flow` tool to run the commit workflow:

    flow_name: "commit"
    parameters:
      message: "$ARGUMENTS"

## What Happens

1. Reviews staged and unstaged changes via `git_changes`
2. Analyzes the diff to understand what changed and why
3. Generates a conventional commit message (feat/fix/refactor/etc.)
4. If $ARGUMENTS provided, uses it as the commit message basis
5. Creates the commit with the formatted message
