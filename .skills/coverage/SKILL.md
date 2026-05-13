---
name: coverage
description: Run tests with coverage instrumentation, identify uncovered code, and produce kanban tasks for coverage gaps. Use when the user says "coverage", "what's untested", "find coverage gaps", or wants to know what needs tests. Automatically delegates to a tester subagent.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for project detection and the `kanban` MCP tool for creating coverage-gap tasks. Also requires a language-appropriate coverage tool on the system PATH (e.g. cargo-llvm-cov for Rust, pytest-cov for Python, go test -cover for Go).
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

# Coverage

Run tests with coverage instrumentation, then identify gaps and produce a concrete work list.

**You MUST run tests with real coverage tools.** Do not guess or structurally deduce coverage — measure it.


When you have poor code coverage, it means you have inherited code that was not done with proper TDD. 
The goal is to fix that and get the test that SHOULD HAVE been written with TDD into place until we get to complete coverage.

** IMPORTANT **  Do not change, delete, modify, refactor the code under test. Improving coverage should only be done by writing new tests, never by changing the code under test. If you find failing tests, note them but do not fix them — that is a separate task.

## Process

### 1. Detect project type and coverage tool

Use `code_context` to detect the project:

```json
{"op": "detect projects"}
```

Read the matching language-specific coverage guide bundled with this skill for exact commands, tool options, scoping flags, test locations, and what requires tests:

| Project type       | Guide |
| ------------------ | ----- |
| Rust (Cargo)       | [RUST_COVERAGE.md](./references/RUST_COVERAGE.md) |
| JS/TS (npm/pnpm)   | [JS_TS_COVERAGE.md](./references/JS_TS_COVERAGE.md) |
| Python (pytest)    | [PYTHON_COVERAGE.md](./references/PYTHON_COVERAGE.md) |
| Dart/Flutter       | [DART_FLUTTER_COVERAGE.md](./references/DART_FLUTTER_COVERAGE.md) |

Follow the guide's instructions for running coverage, installing tools, and scoping. The guide is authoritative — do not guess commands.

### 2. Determine scope

There are two modes — **the user decides which one applies**:

**Explicit scope** (user named specific files, directories, crates, or packages):
- Run coverage ONLY for the specified scope
- Do NOT look at branch changes — the user told you exactly what to analyze
- Example: "coverage for crates/code-context" → run coverage scoped to that crate

**Default scope** (user just said "coverage" with no target):
- Scope to files changed on the current branch vs `main`
- Use `git` with `op: "get changes"` to get the list:
  ```json
  {"op": "get changes"}
  ```

### 3. Run tests with coverage

Run the coverage commands from the language guide. Use the shell tool for all commands. Produce LCOV output — the guide specifies the exact flags and output paths for each tool.

If the coverage tool is not installed, install it using the command in the guide.

If tests fail, note the failures but continue with coverage analysis on the passing tests. Do NOT stop to fix failing tests — that is a separate task.

### 4. Parse coverage data and identify gaps

Read the generated `lcov.info` file. LCOV format reference:

- `SF:<path>` — source file path
- `DA:<line>,<hits>` — line data: line number and execution count
- `DA:<line>,0` means the line was never executed (uncovered)
- `end_of_record` — end of file block

For each file in scope:
- Parse `DA:` lines to get per-line hit counts
- Map uncovered lines (hit count 0) back to functions by reading the source
- Files in scope but absent from coverage data are 0% covered

Compute per-file metrics:
- **Lines instrumented**: count of `DA:` lines for the file
- **Lines covered**: count of `DA:<line>,N` where N > 0
- **Coverage %**: covered / instrumented × 100

### 5. Track coverage gaps on the kanban board

Initialize the board and create a coverage-gap tag:

```json
{"op": "init board"}
```

```json
{"op": "add tag", "id": "coverage-gap", "name": "Coverage Gap", "color": "ff8800", "description": "Function or method lacking test coverage"}
```

Create a kanban task for each uncovered function or block:

```json
{"op": "add task", "title": "Add tests for <function_name>", "description": "<file:lines>\n\nCoverage: <X>% (<covered>/<total> lines)\n\nUncovered lines: <line ranges>\n\n<function signature>\n\n<what it does and what to test>", "tags": ["coverage-gap"]}
```

### 6. Summarize

Report:
- Overall coverage % for files in scope
- Per-file coverage breakdown (file, covered lines, total lines, %)
- Count of kanban tasks created
- Recommendation on where to start writing tests (lowest coverage first)

## Guidelines

- You MUST run tests with coverage instrumentation. Structural deduction alone is not acceptable.
- Do NOT fix failing tests — note them and continue with coverage analysis.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- Report only actionable gaps. Ignore: trivial getters/setters, trait impl boilerplate, generated code.
- If a coverage tool produces no output or errors, fall back to the next tool for that language. If no tool works, report the error clearly.
- If the user wants to write the missing tests, use the implement skill to pick up the kanban tasks.

## Troubleshooting

### Error: `error: no such command: llvm-cov` / `cargo: command not found: llvm-cov`

- **Cause**: `cargo-llvm-cov` (the preferred Rust coverage tool) is not installed on the system. The language guide lists it as the default, but it is not shipped with the Rust toolchain.
- **Solution**: Install it and its LLVM component, then re-run:
  ```
  cargo install cargo-llvm-cov
  rustup component add llvm-tools-preview
  cargo llvm-cov --lcov --output-path lcov.info
  ```
  If the install itself fails (corporate mirrors, no network), fall through to the next tool documented in [RUST_COVERAGE.md](./references/RUST_COVERAGE.md) (e.g. `cargo-tarpaulin`) rather than fabricating coverage numbers.

### Error: `pytest: error: unrecognized arguments: --cov`

- **Cause**: `pytest-cov` is not installed in the active Python environment. `pytest` does not understand `--cov` on its own.
- **Solution**: Install the plugin in the same environment that will run the tests:
  ```
  pip install pytest-cov
  pytest --cov=<package> --cov-report=lcov:lcov.info
  ```
  In a virtualenv-based project, activate the venv first so `pip` writes to the correct site-packages.

### Error: `lcov.info` exists but is empty, or contains no `DA:` lines for files you expected

- **Cause**: The test run did not actually exercise the files you are analyzing — either the tests are filtered out (e.g. `--test-threads` plus a pattern), the files are not compiled into the test binary, or instrumentation failed silently because no binary was rebuilt.
- **Solution**: Force a clean rebuild with instrumentation and verify tests were executed:
  - Rust: `cargo llvm-cov clean --workspace && cargo llvm-cov --lcov --output-path lcov.info`
  - Python: `coverage erase && pytest --cov=<pkg> --cov-report=lcov:lcov.info`
  Then `grep -c '^SF:' lcov.info` — a non-zero count confirms at least one source file was instrumented. If the count is still zero, check that the test command actually ran tests (look for the pass/fail summary).

### Error: coverage numbers drop to 0% for a file you just edited

- **Cause**: The build cache is serving stale instrumented artifacts — common after switching between `cargo test` and `cargo llvm-cov`, which use different rustflags. The new code is not in the instrumented binary.
- **Solution**: Clear the coverage cache and rerun:
  - Rust: `cargo llvm-cov clean --workspace`
  - Python: `coverage erase`
  Then re-run the coverage command from scratch.
