---
name: test
description: Run tests and analyze results. Use when the user wants to run the test suite or test specific functionality. Test runs produce verbose output — automatically delegates to a tester subagent.
metadata:
  author: "swissarmyhammer"
  version: "0.12.11"
---

## Code Quality

- Write clean, readable code that follows existing patterns in the codebase
- Prefer simple, obvious solutions over clever ones
- Make minimal changes to achieve the goal - avoid unnecessary refactoring
- Don't add features, abstractions, or "improvements" beyond what was asked

## Style

- Follow the project's existing conventions for naming, formatting, and structure
- Match the indentation, quotes, and spacing style already in use
- If the project has a formatter config (prettier, rustfmt, black), respect it

## Documentation

- Every function needs a docstring explaining what it does
- Document parameters, return values, and errors
- Update existing documentation if your changes make it stale
- Inline comments explain "why", not "what"

## Error Handling

- Handle errors at appropriate boundaries
- Don't add defensive code for scenarios that can't happen
- Trust internal code and framework guarantees


# Test

**Zero failures. Zero warnings. Zero skipped tests. The build is either clean or it's broken.**

## Process

### 1. Run the full test suite

Run the full test suite for the detected project type. Use the project detection system to determine the correct command.

### 2. Run type checking and linting

Run type checking and linting with warnings as errors (e.g. `cargo clippy -- -D warnings`).

### 3. Check for skipped/ignored tests

Find any skipped or ignored tests. Fix or delete each one — skipped tests are not acceptable.

### 4. Fix every failure and warning

Fix every failure and every warning, re-running after each fix. Understanding why something fails is not the end — it's the start. The reason it fails is the path to making it pass. Follow that path.

### 5. Track failures on the kanban board

Ensure a `test-failure` tag exists:

```json
{"op": "add tag", "id": "test-failure", "name": "Test Failure", "color": "ff0000", "description": "Failing test or type check"}
```

Create kanban cards for each remaining failure:

```json
{"op": "add task", "title": "<concise description>", "description": "<file:lines>\n\n<error message>\n\n<what you tried>", "tags": ["test-failure"]}
```

### 6. Report back

Report: pass/fail, what was fixed, what's left. If you get stuck, report what you tried and where you're blocked — don't silently give up.

## Rules

- ALL tests must pass. A partial pass is a fail.
- ALL compiler and linter warnings must be resolved. Warnings are bugs that haven't bitten yet.
- Skipped tests are not acceptable. A skipped test is either broken (fix it) or dead (delete it).
- Every failing test is your responsibility to fix. No exceptions.
- Do not add `#[allow(...)]`, `@suppress`, `// eslint-disable`, or any other mechanism to silence warnings.
- Do not add `#[ignore]` or `skip` to make a test stop failing.
