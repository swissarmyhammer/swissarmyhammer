---
name: test
description: Run tests and analyze results. Use when the user wants to run the test suite or test specific functionality. Test runs produce verbose output — automatically delegates to a tester subagent.
context: fork
agent: tester
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` MCP tool  for recording test failures as tasks.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/coding-standards" %}

# Test

**Zero failures. Zero warnings. Zero skipped tests. The build is either clean or it's broken.**

## Process

### 1. Run the full test suite

Run the full test suite for the detected project type. Use the project detection system to determine the correct command.

### 2. Run type checking and linting

Run type checking and linting with warnings as errors (e.g. `cargo clippy -- -D warnings`).

### 3. Check for skipped/ignored tests

Find any skipped or ignored tests. Fix or delete each one — skipped tests are not acceptable.

### 4. Fix every failure and warning

Fix every failure and every warning, re-running after each fix. Understanding why something fails is not the end — it's the start. The reason it fails is the path to making it pass. Follow that path.

### 5. Track failures on the kanban board

Ensure a `test-failure` tag exists:

```json
{"op": "add tag", "id": "test-failure", "name": "Test Failure", "color": "ff0000", "description": "Failing test or type check"}
```

Create kanban tasks for each remaining failure:

```json
{"op": "add task", "title": "<concise description>", "description": "<file:lines>\n\n<error message>\n\n<what you tried>", "tags": ["test-failure"]}
```

### 6. Report back

Report: pass/fail, what was fixed, what's left. If you get stuck, report what you tried and where you're blocked — don't silently give up.

## Rules

- ALL tests must pass. A partial pass is a fail.
- ALL compiler and linter warnings must be resolved. Warnings are bugs that haven't bitten yet.
- Skipped tests are not acceptable. A skipped test is either broken (fix it) or dead (delete it).
- Every failing test is your responsibility to fix. No exceptions.
- Do not add `#[allow(...)]`, `@suppress`, `// eslint-disable`, or any other mechanism to silence warnings.
- Do not add `#[ignore]` or `skip` to make a test stop failing.

## Troubleshooting

### Error: a single test hangs and the whole suite never finishes

- **Cause**: The test is waiting on something that will not happen in CI — a network call, a spawned child process, a file watcher, an unflushed buffer, or a deadlocked mutex. Without a timeout, the runner sits forever.
- **Solution**: Run the suite with a hard per-test timeout and isolate the offender. Use the `shell` skill's `timeout` parameter so you can always reclaim control:
  - Rust: `cargo nextest run --test-threads=1 --timeout 60` (or plain `cargo test -- --test-threads=1` under the shell-tool `timeout: 300`)
  - Python: `pytest --timeout=60` (requires `pytest-timeout`)
  - Node: `jest --testTimeout=60000`
  Once you have the failing test name, re-run it alone with `RUST_LOG=trace` / `--verbose` to see where it blocks, fix the underlying wait, and re-run the full suite.

### Error: tests pass locally but fail with "address already in use" or file-not-found when run in parallel

- **Cause**: Tests share mutable global state — the current working directory, an environment variable, a fixed TCP port, or a shared temp file. Parallel runners hit the race.
- **Solution**: Serialize the affected tests with the project's isolation primitive rather than disabling parallelism globally:
  - Rust: `#[serial_test::serial]` on the test function, or use `CurrentDirGuard` / `tempfile::TempDir` so each test gets its own cwd and files
  - Python: `@pytest.mark.serial` with a matching `pytest-xdist` group, or `tmp_path` fixture for filesystem isolation
  - Node: `test.serial(...)` (ava) or fix the port to `0` and read the assigned port back
  Never "fix" this by adding `--test-threads=1` as a permanent flag — that masks the bug.

### Error: a test fails intermittently ("flaky") and passes on retry

- **Cause**: Non-determinism in the test — timing assumptions, unordered iteration (HashMap/HashSet), clock reads, or dependency on external state. Passing "most of the time" is failing.
- **Solution**: Reproduce the failure deterministically before fixing it. Run the single test in a loop with seed logging:
  - Rust: `for i in {1..100}; do cargo test <name> -- --nocapture || break; done`
  - Python: `pytest -x --count=100 <path>::<name>` (requires `pytest-repeat`)
  Once reproduced, remove the source of non-determinism (sort iteration, inject a clock, seed RNGs) rather than adding retries.

### Error: `cargo clippy -- -D warnings` fails with a lint you did not introduce

- **Cause**: A toolchain or dependency bump enabled a new lint, or `clippy` was updated between runs. The warning is real and must be fixed.
- **Solution**: Fix the lint, do not silence it. Start with auto-fixes:
  ```
  cargo clippy --fix --allow-staged --all-targets
  cargo clippy -- -D warnings
  ```
  For lints auto-fix cannot handle, read the `clippy::<lint_name>` documentation (`cargo clippy --explain <lint_name>`) and rewrite the offending code. Never add `#[allow(...)]`.
