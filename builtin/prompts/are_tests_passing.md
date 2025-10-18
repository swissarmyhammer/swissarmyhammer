---
title: are_tests_passing
description: "Check if all tests are passing."
---

## Goal

We want to know if unit tests are passing.

## Rules

If you run tests, on any failure, respond only with NO
If you run tests, and they all pass, respond only with YES
Write failing tests by name to TEST_FAILURES.md

### Rust

- Run tests with `cargo nextest run --fail-fast`
  -- do not try to pass --timeout
