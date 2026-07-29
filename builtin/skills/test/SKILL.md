---
name: test
description: Run the tests and check the results. Use this skill when the user wants to run the test suite or test a specific feature.
agent: tester
license: MIT OR Apache-2.0
compatibility: This skill needs the `kanban` MCP tool. It uses this tool to record test failures as tasks.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Test

**Zero failures. Zero warnings. Zero skipped tests. The build is clean, or it is broken.**


## Guidelines

{% include "_partials/coding-standards" %}
{% include "_partials/architecture-awareness" %}

## Process

1. **Run the full test suite.** Use project detection to pick the correct command.
2. **Type-check and lint the code.** Treat warnings as errors (`cargo clippy -- -D warnings`).
3. **Check for skipped or ignored tests.** Fix each test, or delete it. Do not skip tests.
4. **Fix every failure and warning.** Run the tests again after each fix. Trace the code before you edit it: run `get symbol` on the failing function, and run `get callgraph` (inbound) to find its callers. If you change a shared symbol, run `get blastradius` on the file. This finds passing tests elsewhere that your change could break.
5. **Track the remaining failures on kanban.** Ensure the tag exists:

   ```json
   {"op": "add tag", "id": "test-failure", "name": "Test Failure", "color": "ff0000", "description": "Failing test or type check"}
   ```

   Create one task for each failure:

   ```json
   {"op": "add task", "title": "<concise description>", "description": "<file:lines>\n\n<error>\n\n<what you tried>", "tags": ["test-failure"]}
   ```

6. **Report the results.** State the pass or fail count, what you fixed, and what remains. If you are stuck, state what you tried and where you are blocked.

## Rules

- All tests pass. A partial pass is a fail.
- All warnings are resolved. A warning is a bug that has not happened yet.
- A skipped test is either broken or dead. Fix a broken test. Delete a dead test. Do not leave a test skipped.
- Place new or moved code where `ARCHITECTURE.md` says, if this file exists.
- Do not silence a warning. Do not use `#[allow(...)]`, `@suppress`, or `// eslint-disable`.
- Do not skip a test. Do not use `#[ignore]` or `skip` to stop a test from failing.

## Troubleshooting

### No Tests

Create one test to get started.

### A single test hangs and the suite never finishes

The test waits for something that CI cannot provide, such as network access, a child process, a file watcher, or a deadlock. Run the test with a hard time limit. Use the `timeout` option of the `shell` tool to isolate the failing test:

- Rust: `timeout 60 cargo nextest run --test-threads=1 <test_name>`. Nextest has no `--timeout` flag. You can only set a time limit for each test through `slow-timeout` or `terminate-after` in `.config/nextest.toml`. Wrap the command with the shell `timeout` command to limit one suspect test.
- Python: `pytest --timeout=60`. This needs the `pytest-timeout` package.
- Node: `jest --testTimeout=60000`

Run the failing test again with `RUST_LOG=trace` or `--verbose`. This finds the cause of the wait. Fix the root cause.

### Tests pass locally, fail in parallel ("address in use", missing files)

The tests share mutable state, such as the working directory, an environment variable, a fixed port, or a shared temp file. Use the project's isolation tool to run these tests one at a time. Do not disable parallel tests for the whole suite:

- Rust: use `#[serial_test::serial]`. Use `CurrentDirGuard` or `tempfile::TempDir` for the working directory or files.
- Python: use `@pytest.mark.serial`. Use the `tmp_path` fixture for the file system.
- Node: use `test.serial(...)` (ava). Bind to port `0` and read back the assigned port.

Do not permanently set `--test-threads=1`. This hides the bug.

### Flaky test (passes on retry)

The cause is non-determinism, such as timing, unordered iteration, the clock, or external state. Reproduce the failure in a repeatable way before you fix it:

- Rust: `for i in {1..100}; do cargo test <name> -- --nocapture || break; done`
- Python: `pytest -x --count=100 <path>::<name>` (needs `pytest-repeat`)

Remove the source of the randomness. Sort the iteration order, inject a fixed clock, or seed the random number generator. Do not add retries.

### `cargo clippy -- -D warnings` fails on a lint you did not introduce

A toolchain update enabled a new lint. Fix the code. Do not silence the lint. Try the automatic fix first:

```
cargo clippy --fix --allow-staged --all-targets
cargo clippy -- -D warnings
```

For a lint the automatic fix cannot handle, run `cargo clippy --explain <lint_name>`, then rewrite the code. Do not use `#[allow(...)]`.
