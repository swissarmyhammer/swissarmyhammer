---
title: Test Design
description: How to shape unit and integration tests, and what never to test.
partial: true
---

## Test Design

### DO

- Create small, fast unit tests with a single assertion
- Use fixtures or test setup for shared test initialization

### DO NOT

- Create test that assert the contents of doc strings
- Create tests that scan the source itself and assert on source strings

### Unit vs Integration

Unit tests are small, fast, and focused. An ideal unit test has a single assertion. Use them in TDD.

Integration tests are larger, scenario driven tests involving real data, models, complex setups, and multiple assertion. Do not use them in TDD, use them in CI.

Keep unit tests separated from integration tests in the code. This allows you a faster TDD cycle by focusing only on unit tests.
