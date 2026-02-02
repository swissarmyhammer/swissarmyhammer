---
title: test
description: "Check if all tests and type checks are passing."
---

{% include "_partials/detected-projects" %}
{% include "_partials/test-driven-development" %}


## Goal

Determine if unit tests AND language-specific type checking are passing.

## Steps

1. Run the test suite for the detected project type right now to determine if there are failing tests.
2. Run type checking (e.g., `cargo clippy` for Rust, `tsc` for TypeScript)
3. Create tasks for each and every failure using `kanban` with `op: "add task"`. 
4. Call `cel_set` to record the overall result
- If ALL tests pass: `cel_set(name="are_tests_passing", value=true)`
- If ANY test or type check fails: `cel_set(name="are_tests_passing", value=false)`
5. Summarize the failing tests and type checks in the final output.
