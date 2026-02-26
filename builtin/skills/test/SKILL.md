---
name: test
description: Run tests and analyze results. Use when the user wants to run the test suite or test specific functionality.
metadata:
  author: swissarmyhammer
  version: "1.0"
---


{% include "_partials/detected-projects" %}
{% include "_partials/test-driven-development" %}


## Goal

Determine if unit tests AND language-specific type checking are passing.

## Steps

1. Delegate test execution to the **test** subagent. This keeps verbose test output in the subagent's context rather than cluttering the parent conversation.
2. The subagent will run the test suite and type checks, record results via kanban and js tools, and return a concise summary.
3. Review the subagent's summary and relay results to the user.
