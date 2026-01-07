---
title: test
description: "Check if all tests and type checks are passing."
---

## Goal

We want to know if unit tests are passing AND if language-specific type checking is passing.

## Rules

If you run tests or type checks, on any failure, `cel_set` name `are_tests_passing` to value `false`
If you run tests and type checks, and they all pass, `cel_set` name `are_tests_passing` to value `true`

Create todos for any failures found using the `todo_create` tool.

Each failed test of type check should result in a separate todo.

### Rust

- Run tests with `cargo nextest run --fail-fast` from the root of the workspace to capture all tests
  -- do not try to pass --timeout
- Type checking is done by the compiler during build/test, no separate step needed

### TypeScript/React

- Run tests with the root project's test command (e.g., `npm test` or `yarn test`)
- Run type checking with `npx tsc --noEmit`

### Dart/Flutter

- Run tests with `flutter test` (or `fvm flutter test`)
- Run type checking with `flutter analyze` (or `fvm flutter analyze`)
