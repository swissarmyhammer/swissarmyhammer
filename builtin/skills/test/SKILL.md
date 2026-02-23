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

1. Run the test suite for the detected project type right now to determine if there are failing tests.
2. Run type checking (e.g., `cargo clippy` for Rust, `tsc` for TypeScript)
3. Ensure a `test-failure` tag exists: `kanban` with `op: "add tag"`, `id: "test-failure"`, `name: "Test Failure"`, `color: "ff0000"`, `description: "Failing test or type check"`
4. Create tasks for each and every failure using `kanban` with `op: "add task"`, tagging them: `tags: ["test-failure"]`
5. Use the `js` tool to record the overall result
   - If ALL tests pass: `js` with `op: "set expression"`, `name: "are_tests_passing"`, `expression: "true"`
   - If ANY test or type check fails: `js` with `op: "set expression"`, `name: "are_tests_passing"`, `expression: "false"`
6. Summarize the failing tests and type checks in the final output.
