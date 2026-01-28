---
title: Test Driven Development
description: TDD practices and workflow
partial: true
---

## Test Driven Development

Write tests first, then implementation. This ensures code is testable and requirements are clear.

### TDD Cycle

1. **Red**: Write a failing test that defines what you want
2. **Green**: Write the minimum code to make the test pass
3. **Refactor**: Clean up while keeping tests green

### Guidelines

- Write the test before the implementation
- Each test should verify one behavior
- Run tests frequently - after every small change
- Don't write new code without a failing test first
- If you find a bug, write a test that catches it before fixing
- All tests must pass, there is no such thing as a 'pre existing failure'. If a test is failing, assume you broke it -- because you did and just do not realize it.

### Test Structure

- **Arrange**: Set up the test conditions
- **Act**: Execute the code under test
- **Assert**: Verify the expected outcome

### When to Run Tests

- Before starting work (ensure clean baseline)
- After writing each new test (should fail)
- After writing implementation (should pass)
- Before committing (all tests must pass)
