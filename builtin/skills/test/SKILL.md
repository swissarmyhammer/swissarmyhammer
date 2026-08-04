---
name: test
description: Run tests and analyze results. Use when the user wants to run the test suite or test specific functionality.
agent: tester
license: MIT OR Apache-2.0
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Test

**Zero failures. Zero warnings. Zero skipped. The build is clean or it's broken.**


## Guidelines

- All tests pass. A partial pass is a fail.
- All warnings resolved. Warnings are bugs that haven't bitten yet.
- Skipped tests are broken (fix) or dead (delete) — never acceptable.
- Never silence: no `#[allow(...)]`, `@suppress`, `// eslint-disable`.
- Never skip: no `#[ignore]` or `skip` to make a test stop failing.
- Never hand-prove a test is non-vacuous: edit source, run, watch it fail, revert. Nothing is captured or re-runnable. If proving it needs perturbing the thing under test, that perturbation is a permanent test case: pin the literal value, not just the symbol (`assert_eq!(CONST, "haiku")`, not a compare that resolves both sides through the same symbol), or set an explicit override inside the test and assert the different outcome.

{% include "_partials/findings-are-requirements" %}

## Process

1. **Run the full test suite** using project detection to pick the right command.
2. **Type-check + lint** treating warnings as errors.
3. **Check for skipped/ignored tests** — fix or delete each. Skips are not acceptable.
4. **Fix every failure and warning**, re-running after each fix. Trace before editing: `get symbol` on the failing function, `get callgraph` (inbound) to see callers, and — if you're changing a shared symbol — `get blastradius` on the file to spot passing tests elsewhere that the change could break.
5. **Repeat** until all tests pass.

## Report

{% include "_partials/step-record" %}

Test reports `green`, `red`, or `stuck`. The evidence is the command and its counts.

```
step: test
outcome: green
evidence: cargo nextest run — 1284 passed, 0 failed, 0 skipped; cargo clippy -- -D warnings clean
task: none
```

**Test does not touch the kanban board.** It takes no task id, it writes no comment, and it moves no card. It returns the block, and the caller — `/finish`, `/implement`, or the user — records the outcome on the card. This keeps `/test` usable on a repository with no board.

Report `green` only after a full run passes. A run that was narrowed to one test, or that ended early, is not `green` — say what you ran.

## Troubleshooting

### No Tests

Make one to get started. 

### A single test hangs and the suite never finishes

Test waits on something CI can't deliver (network, child process, file watcher, deadlock). Run with a hard per-test timeout and isolate the offender via the `shell` tool's `timeout`:

- Rust: `timeout 60 cargo nextest run --test-threads=1 <test_name>` — nextest has no `--timeout` flag; its per-test budget is config-only via `slow-timeout`/`terminate-after` in `.config/nextest.toml`, so wrap the invocation with the shell `timeout` to bound a single suspect test
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

### Unrelated Failures

No such thing, your job is to thoughtfully fix all failing tests.
