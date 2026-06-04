---
name: coverage
description: Run tests with coverage instrumentation, identify uncovered code, and produce kanban tasks for coverage gaps. Use when the user says "coverage", "what's untested", "find coverage gaps", or wants to know what needs tests. Automatically delegates to a tester subagent.
agent: tester
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for project detection and the `kanban` MCP tool for creating coverage-gap tasks. Also requires a language-appropriate coverage tool on the system PATH (e.g. cargo-llvm-cov for Rust, pytest-cov for Python, go test -cover for Go).
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Coverage

Run tests with coverage instrumentation, identify gaps, produce a concrete work list.

**Measure — never guess or structurally deduce.**

Poor coverage means inherited code lacks proper TDD. Goal: backfill the tests that *should* have been written, until coverage is complete.

**Do NOT modify the code under test.** Improve coverage only by writing new tests. Failing tests get noted, not fixed — that's a separate task.

## Process

1. **Detect project + tool** — `{"op": "detect projects"}`, then follow the matching guide:

   | Project type | Guide |
   |--------------|-------|
   | Rust (Cargo) | [RUST_COVERAGE.md](./references/RUST_COVERAGE.md) |
   | JS/TS (npm/pnpm) | [JS_TS_COVERAGE.md](./references/JS_TS_COVERAGE.md) |
   | Python (pytest) | [PYTHON_COVERAGE.md](./references/PYTHON_COVERAGE.md) |
   | Dart/Flutter | [DART_FLUTTER_COVERAGE.md](./references/DART_FLUTTER_COVERAGE.md) |

   The guide is authoritative — don't guess commands.

2. **Scope** — user decides:
   - **Explicit** (named files/dirs/crates/packages): coverage only for that scope; ignore branch changes.
   - **Default** ("coverage" with no target): files changed on current branch vs `main`, via `{"op": "get changes"}`.

3. **Run with coverage** — commands from the guide, via the shell tool. Produce LCOV output. Install the tool from the guide if missing. If tests fail, note and continue with the passing ones — don't stop to fix.

4. **Parse LCOV and identify gaps** from `lcov.info`:
   - `SF:<path>` — source file
   - `DA:<line>,<hits>` — line execution count (`0` = uncovered)
   - `end_of_record` — file block end

   For each in-scope file: parse `DA:` lines, map uncovered lines to functions by reading the source. Files in scope but absent from coverage = 0%. Per-file metrics: lines instrumented (`DA:` count), lines covered (`DA:<line>,N>0` count), coverage % = covered/instrumented × 100.

5. **Track on kanban**:

   ```json
   {"op": "init board"}
   {"op": "add tag", "id": "coverage-gap", "name": "Coverage Gap", "color": "ff8800", "description": "Function or method lacking test coverage"}
   {"op": "add task", "title": "Add tests for <function>", "description": "<file:lines>\n\nCoverage: <X>% (<covered>/<total> lines)\n\nUncovered lines: <ranges>\n\n<signature>\n\n<what it does and what to test>", "tags": ["coverage-gap"]}
   ```

6. **Summarize**: overall % for scope, per-file breakdown (file, covered, total, %), kanban task count, recommendation (lowest coverage first).

## Guidelines

- Measure with real coverage instrumentation; no structural deduction.
- Don't fix failing tests — note them.
- Kanban is the single source of truth — no TodoWrite/TaskCreate.
- Report only actionable gaps. Ignore trivial getters/setters, trait-impl boilerplate, generated code.
- Tool error/no-output → fall through to the next tool in the guide; if none work, report clearly.
- To backfill the tests, use the `implement` skill against the kanban tasks.

## Troubleshooting

### `error: no such command: llvm-cov` / `cargo: command not found: llvm-cov`

`cargo-llvm-cov` is not installed. Install both it and the LLVM component:

```
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
cargo llvm-cov --lcov --output-path lcov.info
```

If install fails (corporate mirror, no network), fall through to the next tool in [RUST_COVERAGE.md](./references/RUST_COVERAGE.md) (e.g. `cargo-tarpaulin`) — never fabricate coverage numbers.

### `pytest: error: unrecognized arguments: --cov`

`pytest-cov` not in the active env. Install in the same env that runs the tests:

```
pip install pytest-cov
pytest --cov=<package> --cov-report=lcov:lcov.info
```

In a virtualenv project, activate first.

### `lcov.info` is empty or missing `DA:` for expected files

Tests didn't exercise the files — filtered out, not compiled into the test binary, or instrumentation failed silently. Force a clean rebuild and verify execution:

- Rust: `cargo llvm-cov clean --workspace && cargo llvm-cov --lcov --output-path lcov.info`
- Python: `coverage erase && pytest --cov=<pkg> --cov-report=lcov:lcov.info`

Then `grep -c '^SF:' lcov.info` — non-zero confirms instrumentation. If zero, verify tests actually ran (look for the pass/fail summary).

### Coverage drops to 0% for a file you just edited

Stale instrumented build cache — common after switching between `cargo test` and `cargo llvm-cov` (different rustflags). Clear and rerun:

- Rust: `cargo llvm-cov clean --workspace`
- Python: `coverage erase`
