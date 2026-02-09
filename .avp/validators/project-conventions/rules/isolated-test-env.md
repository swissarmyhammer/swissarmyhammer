---
name: isolated-test-env
description: Tests must use isolated utilities for working directories and environment variables
---

# Isolated Test Environment Rule

Tests that interact with the filesystem or environment variables must use proper isolation utilities to avoid polluting shared state.

## What to Check

Look for tests that:

- Call `std::env::set_current_dir()` without restoring the original
- Call `std::env::set_var()` or `std::env::remove_var()` without cleanup
- Use hardcoded `/tmp` paths instead of `tempfile::TempDir`
- Share mutable global state (working directory, env vars) between tests without `#[serial_test::serial]`

## What Passes

- Tests using `tempfile::TempDir` for temporary directories
- Tests using `serial_test::serial` when mutating global state like cwd or env vars
- Tests that save and restore `current_dir` in a scope guard or cleanup block
- Tests that use `std::env::set_var` with matching cleanup via `std::env::remove_var` and are marked `#[serial_test::serial]`

## What Fails

- `std::env::set_current_dir()` in a test without restoring original directory
- `std::env::set_var()` without corresponding cleanup and without `#[serial_test::serial]`
- Creating files in `/tmp` directly instead of using `TempDir`
- Tests that depend on specific working directory without setting it up
