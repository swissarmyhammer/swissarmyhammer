---
name: coverage
description: Run tests with coverage instrumentation, identify uncovered code, and produce kanban cards for coverage gaps. Use when the user says "coverage", "what's untested", "find coverage gaps", or wants to know what needs tests. Automatically delegates to a tester subagent.
metadata:
  author: "swissarmyhammer"
  version: "0.12.11"
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
| Rust (Cargo)       | [RUST_COVERAGE.md](./RUST_COVERAGE.md) |
| JS/TS (npm/pnpm)   | [JS_TS_COVERAGE.md](./JS_TS_COVERAGE.md) |
| Python (pytest)    | [PYTHON_COVERAGE.md](./PYTHON_COVERAGE.md) |
| Dart/Flutter       | [DART_FLUTTER_COVERAGE.md](./DART_FLUTTER_COVERAGE.md) |

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
