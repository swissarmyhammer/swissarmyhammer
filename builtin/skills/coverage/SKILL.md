---
name: coverage
description: Run tests with coverage instrumentation, identify uncovered code, and produce kanban cards for coverage gaps. Use when the user says "coverage", "what's untested", "find coverage gaps", or wants to know what needs tests. Automatically delegates to a tester subagent.
context: fork
agent: tester
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/detected-projects" %}

# Coverage

Run tests with coverage instrumentation, then identify gaps and produce a concrete work list.

**You MUST run tests with real coverage tools.** Do not guess or structurally deduce coverage — measure it.

## Process

### 1. Detect project type and coverage tool

Use `code_context` to detect the project:

```json
{"op": "detect projects"}
```

Select the coverage tool based on detected project type. See the language-specific coverage guides for exact commands:

| Project type       | Coverage tool                                      |
| ------------------ | -------------------------------------------------- |
| Rust (Cargo)       | `cargo tarpaulin` or `cargo llvm-cov`              |
| JS/TS (npm/pnpm)  | `vitest --coverage` or `jest --coverage`            |
| Python (pytest)    | `pytest --cov`                                     |
| Dart/Flutter       | `flutter test --coverage` or `dart test --coverage` |

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

Run the test suite with coverage instrumentation. Use the shell tool for all commands.

**Produce machine-readable output** — every tool has a format that emits per-file or per-line data:

- Rust: `--out lcov` (tarpaulin) or `--lcov` (llvm-cov) → writes `lcov.info`
- JS/TS: `--coverage --reporter=lcov` → writes `coverage/lcov.info`
- Python: `--cov-report=lcov:lcov.info` → writes `lcov.info`
- Dart/Flutter: `--coverage` → writes `coverage/lcov.info`

If the coverage tool is not installed, install it first (e.g. `cargo install cargo-tarpaulin`).

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

Create a kanban card for each uncovered function or block:

```json
{"op": "add task", "title": "Add tests for <function_name>", "description": "<file:lines>\n\nCoverage: <X>% (<covered>/<total> lines)\n\nUncovered lines: <line ranges>\n\n<function signature>\n\n<what it does and what to test>", "tags": ["coverage-gap"]}
```

### 6. Summarize

Report:
- Overall coverage % for files in scope
- Per-file coverage breakdown (file, covered lines, total lines, %)
- Count of kanban cards created
- Recommendation on where to start writing tests (lowest coverage first)

## Guidelines

- You MUST run tests with coverage instrumentation. Structural deduction alone is not acceptable.
- Do NOT fix failing tests — note them and continue with coverage analysis.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- Report only actionable gaps. Ignore: trivial getters/setters, trait impl boilerplate, generated code.
- If a coverage tool produces no output or errors, fall back to the next tool for that language. If no tool works, report the error clearly.
- If the user wants to write the missing tests, use the implement skill to pick up the kanban cards.
