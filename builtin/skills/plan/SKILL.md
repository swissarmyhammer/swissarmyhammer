---
name: plan
description: Turn specifications into detailed implementation plans with actionable tasks. Use when the user has a spec, feature request, or design document that needs implementation steps.
allowed-tools: "*"
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Plan

Create a comprehensive implementation plan from a specification file.

## How to Execute

Use the `flow` tool to run the planning workflow:

    flow_name: "plan"
    parameters:
      plan_filename: "$ARGUMENTS"

## What Happens

1. Reads and analyzes the specification file
2. Reviews existing codebase architecture
3. Creates a draft plan at `.swissarmyhammer/tmp/DRAFT_PLAN.md`
4. Generates actionable todo items on the kanban board
