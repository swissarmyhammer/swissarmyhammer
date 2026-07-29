---
name: coverage
description: Run tests with coverage instrumentation, identify uncovered code, and produce kanban tasks for coverage gaps. Use this skill when the user says "coverage", "what's untested", "find coverage gaps", or wants to know what needs tests.
agent: tester
license: MIT OR Apache-2.0
compatibility: This skill requires the `code_context` MCP tool for project detection and the `kanban` MCP tool to create coverage-gap tasks. It also requires a language-appropriate coverage tool on the system PATH — for example, cargo-llvm-cov for Rust, pytest-cov for Python, go test -cover for Go, or swift test --enable-code-coverage plus llvm-cov for Swift.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Coverage

Run tests with coverage instrumentation. Identify gaps. Produce a concrete work list.


**Measure coverage. Never guess it or deduce it structurally.**

Poor coverage means inherited code lacks proper TDD. The goal is to backfill the tests that *should* have been written, until coverage is complete.

**Do NOT modify the code under test.** Improve coverage only by writing new tests. Note failing tests; do not fix them — fixing them is a separate task.

## Process

1. **Detect the project and tool** — run `{"op": "detect projects"}`, then follow the matching guide:

   | Project type | Guide |
   |--------------|-------|
   | Rust (Cargo) | [RUST_COVERAGE.md](./references/RUST_COVERAGE.md) |
   | JS/TS (npm/pnpm) | [JS_TS_COVERAGE.md](./references/JS_TS_COVERAGE.md) |
   | Python (pytest) | [PYTHON_COVERAGE.md](./references/PYTHON_COVERAGE.md) |
   | Dart/Flutter | [DART_FLUTTER_COVERAGE.md](./references/DART_FLUTTER_COVERAGE.md) |
   | Swift (SwiftPM/Xcode) | [SWIFT_COVERAGE.md](./references/SWIFT_COVERAGE.md) |

   The guide is authoritative — do not guess commands.

2. **Scope** — the user decides:
   - **Explicit** (named files/dirs/crates/packages): measure coverage only for that scope; ignore branch changes.
   - **Default** ("coverage" with no target): measure the files changed on the current branch versus `main`, via `{"op": "get changes"}`.

3. **Run with coverage** — use the commands from the guide, through the shell tool. Produce LCOV output. Install the tool from the guide if it is missing. If tests fail, note this and continue with the passing ones — do not stop to fix them.

4. **Parse LCOV and identify gaps** from `lcov.info`:
   - `SF:<path>` — source file
   - `DA:<line>,<hits>` — line execution count (`0` means uncovered)
   - `end_of_record` — end of the file block

   For each in-scope file: parse the `DA:` lines, and map uncovered lines to functions by reading the source. A file in scope but absent from coverage counts as 0%. Per-file metrics: lines instrumented (`DA:` count), lines covered (`DA:<line>,N>0` count), coverage % = covered/instrumented × 100.

5. **Track on kanban**:

   ```json
   {"op": "init board"}
   {"op": "add tag", "id": "coverage-gap", "name": "Coverage Gap", "color": "ff8800", "description": "Function or method lacking test coverage"}
   {"op": "add task", "title": "Add tests for <function>", "description": "<file:lines>\n\nCoverage: <X>% (<covered>/<total> lines)\n\nUncovered lines: <ranges>\n\n<signature>\n\n<what it does and what to test>", "tags": ["coverage-gap"]}
   ```

6. **Summarize**: report the overall % for the scope, a per-file breakdown (file, covered, total, %), the kanban task count, and a recommendation to fix the lowest coverage first.

## Guidelines

- Measure with real coverage instrumentation; do not deduce coverage structurally.
- Do not fix failing tests — note them.
- Kanban is the single source of truth — do not use TodoWrite or TaskCreate.
- Report only actionable gaps. Ignore trivial getters/setters, trait-impl boilerplate, generated code.
- If a tool errors or produces no output, fall through to the next tool in the guide; if none work, report this clearly.
- To backfill the tests, use the `implement` skill against the kanban tasks.

## Troubleshooting

### `error: no such command: llvm-cov` / `cargo: command not found: llvm-cov`

`cargo-llvm-cov` is not installed. Install it and the LLVM component:

```
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
cargo llvm-cov --lcov --output-path lcov.info
```

If the install fails (corporate mirror, no network), fall through to the next tool in [RUST_COVERAGE.md](./references/RUST_COVERAGE.md) (e.g. `cargo-tarpaulin`) — never fabricate coverage numbers.

### `pytest: error: unrecognized arguments: --cov`

`pytest-cov` is not in the active environment. Install it in the same environment that runs the tests:

```
pip install pytest-cov
pytest --cov=<package> --cov-report=lcov:lcov.info
```

In a virtualenv project, activate the environment first.

### `lcov.info` is empty or missing `DA:` for expected files

The tests did not exercise the files — filtered out, not compiled into the test binary, or instrumentation failed silently. Force a clean rebuild and verify execution:

- Rust: `cargo llvm-cov clean --workspace && cargo llvm-cov --lcov --output-path lcov.info`
- Python: `coverage erase && pytest --cov=<pkg> --cov-report=lcov:lcov.info`
- Swift: `rm -rf .build && swift test --enable-code-coverage`

Then run `grep -c '^SF:' lcov.info` — a non-zero result confirms instrumentation. If it is zero, verify the tests actually ran (look for the pass/fail summary).

### Coverage drops to 0% for a file you just edited

This is a stale instrumented build cache, common after switching between `cargo test` and `cargo llvm-cov` (different rustflags). Clear the cache and rerun:

- Rust: `cargo llvm-cov clean --workspace`
- Python: `coverage erase`
- Swift: `rm -rf .build/*/debug/codecov` then rerun `swift test --enable-code-coverage` (a plain `swift test` run leaves a profdata that no longer matches the binary)

### Swift: `llvm-cov: command not found` or `Failed to load coverage: unsupported instrumentation profile format version`

Use the llvm-cov that matches the Swift toolchain, not a Homebrew LLVM: on macOS always invoke it as `xcrun llvm-cov`; on Linux use the one in the Swift toolchain's `usr/bin`. A version-mismatch error means a foreign llvm-cov read the profdata — use the same fix.
