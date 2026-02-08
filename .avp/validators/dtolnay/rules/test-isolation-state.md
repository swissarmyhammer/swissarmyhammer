---
name: test-isolation-state
description: Tests that touch files, directories, or any stored state must use a shared RAII test environment type
---

# Test Isolation for Stored State

Tests that read or write files, directories, databases, environment variables,
or any persistent state are the most common source of flaky failures. dtolnay's
approach: build an RAII type that encapsulates the isolated environment, put it
in a shared workspace crate, and use it everywhere.

## The TestEnv Pattern

The workspace should have a shared test-utilities crate (e.g., `test-support`,
`test-harness`, or similar) that provides a `TestEnv` type (or equivalent) with
these properties:

- **RAII construction**: `TestEnv::new()` creates a unique temporary directory,
  sets up any needed fixture state, and returns an owned handle.
- **Drop cleanup**: `impl Drop for TestEnv` removes the temp directory and
  restores any modified state. Cleanup happens even if the test panics.
- **Path access**: Exposes a `path()` method returning the root of the isolated
  directory. All file operations in the test are scoped under this path.
- **Fixture helpers**: Methods to write fixture files, create subdirectories,
  or populate initial state within the isolated environment.
- **No global mutation**: The type does not call `std::env::set_var`,
  `std::env::set_current_dir`, or write to any `static`. Configuration that
  the code under test needs is passed explicitly or scoped to the `TestEnv`.

Look for this pattern in the workspace before creating a new one. If it exists,
tests must use it rather than rolling their own temp directory logic. If it
doesn't exist and a test needs filesystem isolation, that's a signal to create it.

## What to Check

**FIRST ACTION: Search the workspace for an existing test environment type.**

Look for a shared crate or module that provides a `TestEnv`, `TestDir`,
`TestContext`, or similar RAII harness. Common locations: crates named
`test-support`, `test-harness`, `test-utils`, or `*-testing`; `impl Drop`
on types used in test code; `dev-dependencies` that point to a workspace
member. If one exists, the test must use it -- not reinvent it.

If none exists and the test needs filesystem isolation, flag this as an
architectural gap: the workspace needs a shared test environment crate.

1. **Raw tempdir usage in tests**: Tests calling `tempfile::tempdir()` directly
   instead of using the workspace's shared test environment type. Ad-hoc tempdir
   usage duplicates setup/teardown logic and drifts over time.

2. **Shared file paths**: Tests using hardcoded paths like `/tmp/test_output`
   or `./test_data.json`. Every path must come from the `TestEnv`.

3. **Missing the shared crate**: If a workspace has multiple crates with tests
   that touch the filesystem, and there is no shared test-utilities crate, flag
   this as an architectural gap.

4. **Environment variable mutation**: Tests calling `std::env::set_var` without
   isolation. Environment variables are process-global -- setting one in a test
   affects all concurrent tests. The code under test should accept config as a
   parameter, not read it from the environment directly.

5. **Working directory changes**: Tests calling `std::env::set_current_dir`.
   Process-global, breaks parallel tests. The `TestEnv` provides a path --
   pass it explicitly.

6. **Static or global mutable state**: Tests writing to `static mut`,
   `lazy_static`, `once_cell`, or any global mutable. If a component needs
   mutable state, it should accept it as a parameter so tests can provide
   isolated instances.

7. **Port or socket conflicts**: Tests binding to a hardcoded port. Use port 0
   to let the OS assign an available port.

8. **File existence assumptions**: Tests that assume a file from a previous test
   or from the build. The `TestEnv` should create everything it needs.

## What Passes

- A shared crate in the workspace providing `TestEnv` (or similar) with Drop
- Tests that start with `let env = TestEnv::new()?;` and scope all IO under
  `env.path()`
- `TestEnv` helpers like `env.write_fixture("config.toml", contents)?`
- Code under test that accepts a root path or config struct rather than
  reading from the global environment
- Port 0 for network tests: `TcpListener::bind("127.0.0.1:0")?`

## What Fails

- Bare `tempfile::tempdir()` calls scattered across multiple crates when a
  shared `TestEnv` exists or should exist
- Two tests writing to the same file path without unique namespacing
- `std::fs::write("output.txt", data)` in a test without using the test env
- `std::env::set_var(...)` or `std::env::set_current_dir(...)` in a test
- A test that passes alone but fails under parallel execution
- `static mut` or global state written by one test and read by another
- Manual `std::fs::remove_dir_all` cleanup instead of relying on Drop
- `TcpListener::bind("127.0.0.1:8080")` with a hardcoded port
- Duplicated test setup logic across crates that should share a harness

## Why This Matters

`cargo test` runs tests in parallel by default. Flaky tests from shared state
are the number one reason teams add `--test-threads=1`, which makes the suite
slow and hides the real problem. dtolnay would never let each test reinvent
temp directory management. He'd write the type once, put it in a shared crate,
derive `Drop`, and every test in the workspace would use it. One correct
implementation, used everywhere, tested once.
