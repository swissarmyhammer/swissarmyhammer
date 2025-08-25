---
title: test
description: Iterate to correct test failures in the codebase.
---

## Goals

The goal is to have:

- ALL tests pass

## Rules

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

- Run tests with `cargo nextest run`

## Process

- run all tests
- look at modified files on your current branch and figure out if you are resuming interrupted work
- write all errors and warnings to a markdown scratchpad file `./TEST_FAILURES.md`, this is your todo list
- if there is an existing `./TEST_FAILURES.md`, just append to it -- more work to do!
- when you start to work on a specific test, use the notify_create tool to let the user know
- when you fix a specific test, use the notify_create tool to let the user know
- DO NOT commit to git
- DO NOT mark the issue complete with the issue_mark_complete tool
{% render "todo", todo_file: "./TEST_FAILURES.md" %}

## Reporting

Describe what you plan to do to fix each failing test in this format:

<failing test name>:
- [ ] todo step 1
- [ ] todo step 2
...

Show overall test results as:

✅ <number passed> / <total tests>, if all tests pass
🛑 <number passed> / <total tests>, if there are any failures