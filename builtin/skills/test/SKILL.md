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

**Zero failures. Zero warnings. Zero skipped tests. The build is either clean or it's broken.**

## Rules

- ALL tests must pass. A partial pass is a fail.
- ALL compiler and linter warnings must be resolved. Warnings are bugs that haven't bitten yet.
- Skipped tests are not acceptable. A skipped test is either broken (fix it) or dead (delete it). Decide which and act.
- Every failing test is your responsibility to fix. No exceptions.
- Every warning is your responsibility to fix. No exceptions.
- Understanding why something fails is not the end — it's the start. The reason it fails is the path to making it pass. Follow that path.
- Do not add `#[allow(...)]`, `@suppress`, `// eslint-disable`, or any other mechanism to silence warnings. Fix the underlying issue.
- Do not add `#[ignore]` or `skip` to make a test stop failing. That is hiding the problem, not solving it.

## Steps

1. Run the **full** test suite for the detected project type. Do not cherry-pick or filter tests.
2. Run type checking and linting with warnings treated as errors:
   - Rust: `cargo clippy -- -D warnings`
   - TypeScript: `tsc --noEmit`
   - Python: linter with all warnings enabled
3. Check for skipped/ignored tests. For each one, determine: is it fixable or dead? Fix or delete.
4. If there are ANY failures or warnings:
   a. Ensure a `test-failure` tag exists: `kanban` with `op: "add tag"`, `id: "test-failure"`, `name: "Test Failure"`, `color: "ff0000"`, `description: "Failing test or type check"`
   b. Create tasks for each failure and each warning using `kanban` with `op: "add task"`, tagging them: `tags: ["test-failure"]`
   c. **Fix every single one.** Re-run the suite after each fix to confirm.
5. Repeat until the entire suite is clean — zero failures, zero warnings, zero skipped.
6. Summarize the results in the final output.
