# Implement isolated test environment setup for prompt resolver

## Location
- `swissarmyhammer-prompts/src/prompt_resolver.rs:121`
- `swissarmyhammer-prompts/src/prompt_resolver.rs:231`
- `swissarmyhammer-prompts/src/prompt_resolver.rs:267`

## Current State
```rust
// Skip isolated test environment setup for now
```

## Description
Multiple tests in the prompt resolver skip isolated test environment setup. This should be properly implemented to ensure tests don't interfere with each other or the user's environment.

## Requirements
- Implement proper test isolation for prompt resolver tests
- Create temporary directories for test prompts
- Mock file system access where appropriate
- Ensure cleanup after tests
- Prevent tests from modifying user's actual prompts
- Add documentation for test setup patterns

## Impact
Tests may interfere with each other or user's environment, causing flaky tests.