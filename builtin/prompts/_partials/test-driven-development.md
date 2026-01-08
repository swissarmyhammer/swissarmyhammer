---
title: Test Driven Development
description: TDD practices and workflow
partial: true
---

## Test Driven Development

Write tests first, then implementation. This ensures code is testable and requirements are clear.

### TDD Cycle

1. **Red**: Write a failing test that defines what you want
2. **Green**: Write the minimum code to make the test pass
3. **Refactor**: Clean up while keeping tests green

### Guidelines

- Write the test before the implementation
- Each test should verify one behavior
- Run tests frequently - after every small change
- Don't write new code without a failing test first
- If you find a bug, write a test that catches it before fixing

### Test Structure

- **Arrange**: Set up the test conditions
- **Act**: Execute the code under test
- **Assert**: Verify the expected outcome

### When to Run Tests

- Before starting work (ensure clean baseline)
- After writing each new test (should fail)
- After writing implementation (should pass)
- Before committing (all tests must pass)

### Rust

- Run tests with `cargo nextest run --fail-fast` from the root of the workspace to capture all tests
  -- do not try to pass --timeout
- DO NOT `cd` into a directory to test, use the workspace root, use command line switches to limit to crates
- Type checking is done by the compiler during build/test, no separate step needed

### TypeScript/React

- Run tests with the root project's test command (e.g., `npm test` or `yarn test`)
- Run type checking with `npx tsc --noEmit`

### Dart/Flutter

- Run tests with `flutter test` (or `fvm flutter test`)
- Run type checking with `flutter analyze` (or `fvm flutter analyze`)
