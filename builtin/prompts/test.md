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
