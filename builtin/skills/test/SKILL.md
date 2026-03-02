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

**ALL tests and type checks MUST pass. Zero failures. No exceptions.**

## Rules

- ALL tests must pass. A partial pass is a fail.
- Every failing test is your responsibility to fix. No exceptions.
- Understanding why a test fails is not the end — it's the start. The reason it fails is the path to making it pass. Follow that path.

## Steps

1. Run the **full** test suite for the detected project type. Do not cherry-pick or filter tests.
2. Run type checking (e.g., `cargo clippy` for Rust, `tsc` for TypeScript)
3. If there are ANY failures:
   a. Ensure a `test-failure` tag exists: `kanban` with `op: "add tag"`, `id: "test-failure"`, `name: "Test Failure"`, `color: "ff0000"`, `description: "Failing test or type check"`
   b. Create tasks for each and every failure using `kanban` with `op: "add task"`, tagging them: `tags: ["test-failure"]`
   c. **Fix every failure.** Re-run the suite after each fix to confirm.
4. Repeat until the entire suite is green — zero failures.
5. Summarize the results in the final output.
