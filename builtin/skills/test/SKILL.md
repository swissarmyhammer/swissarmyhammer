---
name: test
description: Run tests and analyze results. Use when the user wants to run the test suite or test specific functionality. Test runs produce verbose output — consider delegating to a test-focused agent to keep the main context clean.
metadata:
  author: swissarmyhammer
  version: "2.0"
---

## Goal

**Zero failures. Zero warnings. Zero skipped tests. The build is either clean or it's broken.**

## Process

Delegate test execution to a **tester** subagent. This keeps verbose test output, compiler errors, and fix iterations in the subagent's context instead of cluttering yours.

### 1. Spawn a tester subagent

Tell it to:
- Run the full test suite for the detected project type
- Run type checking and linting with warnings as errors (e.g. `cargo clippy -- -D warnings`)
- Check for skipped/ignored tests — fix or delete each one
- Fix every failure and every warning, re-running after each fix
- Report back: pass/fail, what was fixed, what's left

### 2. Review the results

When the subagent returns:
- If everything passed: report the clean result to the user
- If there are remaining failures: create kanban cards for each one using `kanban` with `op: "add task"`, tagged `["test-failure"]`

## Rules

- ALL tests must pass. A partial pass is a fail.
- ALL compiler and linter warnings must be resolved. Warnings are bugs that haven't bitten yet.
- Skipped tests are not acceptable. A skipped test is either broken (fix it) or dead (delete it).
- Every failing test is your responsibility to fix. No exceptions.
- Understanding why something fails is not the end — it's the start. The reason it fails is the path to making it pass. Follow that path.
- Do not add `#[allow(...)]`, `@suppress`, `// eslint-disable`, or any other mechanism to silence warnings.
- Do not add `#[ignore]` or `skip` to make a test stop failing.
