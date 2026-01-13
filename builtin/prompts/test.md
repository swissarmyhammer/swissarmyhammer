---
title: test
description: "Check if all tests and type checks are passing."
---

{% include "_partials/detected-projects" %}

## Goal

We want to know if unit tests are passing AND if language-specific type checking is passing.

## Important Instructions

- Use the `cel_set` tool to set the variable `are_tests_passing`
- If ANY tests or type checks fail: call `cel_set` with name="are_tests_passing" and value="false"
- If ALL tests and type checks pass: call `cel_set` with name="are_tests_passing" and value="true"
- Create todos for any failures found using the `todo_create` tool
- Each failed test or type check should result in a separate todo
