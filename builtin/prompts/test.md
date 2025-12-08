---
title: test
description: Iterate to correct test failures and type errors in the codebase.
---

<!-- sah rule ignore test_rule_with_allow -->

## Goals

The goal is to have:

- ALL tests pass
- ALL type checks pass

## Rules

- NEVER ignore tests. NEVER ignore tests. NEVER ignore tests.
- NEVER ignore type errors. Fix all type checking failures.
- Always run tests using a command line tool appropriate for the project
  - start with a 5 minute timeout
- Always run language-specific type checking after running tests
- YOU MUST debug the code to make all tests pass AND all type checks pass, only change tests as a last resort
- If individual tests are identified as slow
  - check if any tests are hanging and correct them
  - speed the tests up so they are no longer slow
    - this may require decomposing a large slow test into multiple smaller tests
- Corrections should be constructive
  - do not comment out or ignore failing tests
  - do not use type assertions or 'any' to bypass type errors
- Feel free to refactor

### Rust

- Run tests with `cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail`
- Be patient and let the test run finish before moving on
- Type checking happens during compilation, no separate step needed

### TypeScript/React

- Run tests with the project's test command (e.g., `npm test` or `yarn test`)
- Run type checking with `npx tsc --noEmit`
- Fix all type errors before considering the work complete

### Dart/Flutter

- Run tests with `flutter test` (or `fvm flutter test`)
- Run type checking with `flutter analyze` (or `fvm flutter analyze`)
- Fix all analysis errors and warnings before considering the work complete

## Process

- run all tests
- run language-specific type checking
- read the modified files on your current branch to establish context
- write all test failures, type errors, slowness, and warnings to a markdown scratchpad file `.swissarmyhammer/tmp/TEST_FAILURES.md`, this is your todo list of things to fix
- refer to `.swissarmyhammer/tmp/TEST_FAILURES.md` to refresh your memory
- if there is an existing `.swissarmyhammer/tmp/TEST_FAILURES.md`, read it, think, and append to it -- more work to do!
- fix broken tests and type errors one at a time, focus and don't get distracted
- DO NOT commit to git
{% render "todo", todo_file: ".swissarmyhammer/tmp/TEST_FAILURES.md" %}
