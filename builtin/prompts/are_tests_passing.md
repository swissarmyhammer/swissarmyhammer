---
title: are_tests_passing
description: "Check if all tests and type checks are passing."
---

## Goal

We want to know if unit tests are passing AND if language-specific type checking is passing.

## Rules

If you run tests or type checks, on any failure, respond only with NO
If you run tests and type checks, and they all pass, respond only with YES
Write failing tests by name to TEST_FAILURES.md

### Rust

- Run tests with `cargo nextest run --fail-fast`
  -- do not try to pass --timeout
- Type checking is done by the compiler during build/test, no separate step needed

### TypeScript/React

- Run tests with the project's test command (e.g., `npm test` or `yarn test`)
- Run type checking with `npx tsc --noEmit`
- Both tests AND type checking must pass to respond YES

### Dart/Flutter

- Run tests with `flutter test` (or `fvm flutter test`)
- Run type checking with `flutter analyze` (or `fvm flutter analyze`)
- Both tests AND analysis must pass to respond YES
