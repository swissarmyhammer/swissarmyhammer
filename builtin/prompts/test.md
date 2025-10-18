---
title: test
description: Iterate to correct test failures in the codebase.
---

## Goals

The goal is to have:

- ALL tests pass

## Rules

- NEVER ignore tests. NEVER ignore tests. NEVER ignore tests.
- Always run tests using a command line tool appropriate for the project
  - start with a 5 minute timeout
- YOU MUST debug the code to make all tests pass, only change tests as a last resort
- If individual tests are identified as slow
  - check if any tests are hanging and correct them
  - speed the tests up so they are no longer slow
    - this may require decomposing a large slow test into multiple smaller tests
- Corrections should be constructive
  - do not comment out or ignore failing tests
- Feel free to refactor

### Rust

- Run tests with `cargo nextest run --failure-output immediate --hide-progress-bar --status-level fail --final-status-level fail`
- Be patient and let the test run finish before moving on

## Process

- run all tests
- read the modified files on your current branch to establish context
- write all test failures, errors, slowness, and warnings to a markdown scratchpad file `.swissarmyhammer/tmp/TEST_FAILURES.md`, this is your todo list of things to fix
- refer to `.swissarmyhammer/tmp/TEST_FAILURES.md` to refresh your memory
- if there is an existing `.swissarmyhammer/tmp/TEST_FAILURES.md`, read it, think, and append to it -- more work to do!
- fix broken tests one at a time, focus and don't get distracted
- DO NOT commit to git
- DO NOT mark the issue complete with the issue_mark_complete tool
{% render "todo", todo_file: ".swissarmyhammer/tmp/TEST_FAILURES.md" %}

## Reporting

Show overall test results as:

✅ <number passed> / <total tests>, if all tests pass
🛑 <number passed> / <total tests>, if there are any failures
