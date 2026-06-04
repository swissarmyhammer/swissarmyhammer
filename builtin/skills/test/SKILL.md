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

# Test

**Zero failures. Zero warnings. Zero skipped. The build is clean or it's broken.**


## Guidelines

{% include "_partials/coding-standards" %}
{% include "_partials/architecture-awareness" %}

## Process

1. **Run the full test suite** using project detection to pick the right command.
2. **Type-check + lint** with warnings as errors (`cargo clippy -- -D warnings`).
3. **Check for skipped/ignored tests** — fix or delete each. Skips are not acceptable.
4. **Fix every failure and warning**, re-running after each fix. Trace before editing: `get symbol` on the failing function, `get callgraph` (inbound) to see callers, `get blastradius` on the file to avoid breaking a passing test elsewhere.
5. **Track remaining failures on kanban.** Ensure tag exists:

   ```json
   {"op": "add tag", "id": "test-failure", "name": "Test Failure", "color": "ff0000", "description": "Failing test or type check"}
   ```

   Create one task per failure:

   ```json
   {"op": "add task", "title": "<concise description>", "description": "<file:lines>\n\n<error>\n\n<what you tried>", "tags": ["test-failure"]}
   ```

6. **Report**: pass/fail, what was fixed, what's left. If stuck, say what you tried and where you're blocked.

## Rules

- All tests pass. A partial pass is a fail.
- All warnings resolved. Warnings are bugs that haven't bitten yet.
- Skipped tests are broken (fix) or dead (delete) — never acceptable.
- Place new/relocated code per `ARCHITECTURE.md` if one exists.
- Never silence: no `#[allow(...)]`, `@suppress`, `// eslint-disable`.
- Never skip: no `#[ignore]` or `skip` to make a test stop failing.

## Troubleshooting

### A single test hangs and the suite never finishes

Test waits on something CI can't deliver (network, child process, file watcher, deadlock). Run with a hard per-test timeout and isolate the offender via the `shell` tool's `timeout`:

- Rust: `cargo nextest run --test-threads=1 --timeout 60`
- Python: `pytest --timeout=60` (needs `pytest-timeout`)
- Node: `jest --testTimeout=60000`

Re-run the offending test with `RUST_LOG=trace` / `--verbose` to find the wait, fix the underlying cause.

### Tests pass locally, fail in parallel ("address in use", missing files)

Tests share mutable state — cwd, env var, fixed port, shared temp file. Serialize with the project's isolation primitive, don't disable parallelism globally:

- Rust: `#[serial_test::serial]`; `CurrentDirGuard` / `tempfile::TempDir` for cwd/files
- Python: `@pytest.mark.serial`; `tmp_path` fixture for filesystem
- Node: `test.serial(...)` (ava); bind port `0` and read it back

Never permanently set `--test-threads=1` — it masks the bug.

### Flaky test (passes on retry)

Non-determinism — timing, unordered iteration, clock, external state. Reproduce deterministically before fixing:

- Rust: `for i in {1..100}; do cargo test <name> -- --nocapture || break; done`
- Python: `pytest -x --count=100 <path>::<name>` (needs `pytest-repeat`)

Remove the source (sort iteration, inject a clock, seed RNGs) — don't add retries.

### `cargo clippy -- -D warnings` fails on a lint you didn't introduce

Toolchain bump enabled a new lint. Fix, don't silence. Auto-fix first:

```
cargo clippy --fix --allow-staged --all-targets
cargo clippy -- -D warnings
```

For lints auto-fix can't handle: `cargo clippy --explain <lint_name>`, rewrite the code. Never `#[allow(...)]`.
