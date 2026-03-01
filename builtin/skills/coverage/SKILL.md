---
name: coverage
description: Analyze test coverage gaps on changed code. Scans branch changes, maps functions to tests structurally, and produces kanban cards for untested code. Use when the user says "coverage", "what's untested", "find coverage gaps", or wants to know what needs tests.
metadata:
  author: swissarmyhammer
  version: "1.0"
---


{% include "_partials/detected-projects" %}


## Goal

Identify test coverage gaps in changed code and produce a concrete work list of what needs tests.

## Steps

1. Delegate coverage analysis to the **coverage** subagent. This keeps verbose analysis (AST queries, file-by-file scanning) out of the parent context.
2. The subagent will scope to the branch diff, structurally analyze what's tested and what's not, create kanban cards for gaps, and return a concise summary.
3. Review the subagent's summary and relay results to the user.
