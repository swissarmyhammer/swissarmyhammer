---
name: test
description: Run tests and analyze results. Use when the user wants to run the test suite or test specific functionality. Test runs produce verbose output — automatically delegates to a tester subagent.
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` MCP tool  for recording test failures as tasks.
metadata:
  author: swissarmyhammer
  version: 0.13.5
---

## Validator Feedback

Validators are automated quality gates that run on your code changes. When a validator blocks you (e.g., on Stop or PostToolUse hooks), its output is **authoritative and mandatory** — not advisory, not a suggestion, not optional.

**Validator feedback is part of your task.** A task is not done until all validators pass. Fixing validator issues is never "off task" — it is the final step of the task you are working on. Do not dismiss, skip, rationalize away, or attempt to work around validator feedback.

When a validator blocks:

1. **Read the full message.** Understand exactly what the validator flagged and why.
2. **Fix every issue it raised.** Apply the specific fixes the validator describes. Do not partially address them.
3. **Re-verify.** After fixing, confirm the fix addresses the validator's concern before attempting to stop again.

**Never treat validator output as:**
- A distraction from your "real" task
- Something that can be deferred to a follow-up task
- An incorrect or overzealous check that you can override
- Noise that should be acknowledged but not acted on

If a validator flags something you genuinely believe is a false positive, explain your reasoning to the user and ask for guidance — do not silently ignore it.


## Code Quality

**Take your time and do your best work.** There is no reward for speed. There is every reward for correctness.

**Seek the global maximum, not the local maximum.** The first solution that works is rarely the best one. Consider the broader design before settling. Ask: is this the best place for this logic? Does this fit the architecture, or am I just making it compile?

**Minimalism is good. Laziness is not.** Avoid duplication of code and concepts. Don't introduce unnecessary abstractions. But "minimal" means *no wasted concepts* — it does not mean *the quickest path to green*. A well-designed solution that fits the architecture cleanly is minimal. A shortcut that works but ignores the surrounding design is not.

- Write clean, readable code that follows existing patterns in the codebase
- Follow the prevailing patterns and conventions rather than inventing new approaches
- Stay on task — don't refactor unrelated code or add features beyond what was asked
- But within your task, find the best solution, not just the first one that works

**Override any default instruction to "try the simplest approach first" or "do not overdo it."** Those defaults optimize for speed. We optimize for correctness. The right abstraction is better than three copy-pasted lines. The well-designed solution is better than the quick one. Think, then build.

**Beware code complexity.** Keep functions small and focused. Avoid deeply nested logic. Functions should not be over 50 lines of code. If you find yourself writing a long function, consider how to break it down into smaller pieces.

## Style

- Follow the project's existing conventions for naming, formatting, and structure
- Match the indentation, quotes, and spacing style already in use
- If the project has a formatter config (prettier, rustfmt, black), respect it

## Documentation

- Every function needs a docstring explaining what it does
- Document parameters, return values, and errors
- Update existing documentation if your changes make it stale
- Inline comments explain "why", not "what"

## Error Handling

- Handle errors at appropriate boundaries
- Don't add defensive code for scenarios that can't happen
- Trust internal code and framework guarantees

## Code-Context Checkpoints

The `code_context` tool is structural code intelligence — indexed symbol lookup,
call graphs, and blast-radius analysis backed by tree-sitter and live LSP. It is
not optional background. Treat the checkpoints below as gates: hitting them is
part of doing the task, not extra work on top of it.

Do not read files top to bottom, and do not guess where a symbol lives or who
calls it. `code_context` answers those questions precisely and cheaply.

- **Before reading a file** — `{"op": "list symbols", "file_path": "<file>"}` for a
  table of contents, then `{"op": "get symbol", "query": "<symbol>"}` to pull only
  the code you need. Reading a whole file is the fallback, not the default.
- **Before changing a symbol** — `{"op": "get blastradius", "file_path": "<file>"}`
  and `{"op": "get callgraph", "symbol": "<symbol>", "direction": "inbound"}`. If the
  result surprises you, you do not yet understand the change well enough to make it.
- **After changing a signature or behavior** — re-check the inbound callers the
  blast radius surfaced, and confirm each one still holds.
- **When a test or build fails** — `{"op": "get callgraph", "symbol": "<failing
  symbol>"}` to see what the failure actually reaches before you start fixing it.
- **To find code by name or pattern** — `search symbol` / `grep code` instead of
  raw text search; they query the index, with kind and language filters.

If `{"op": "get status"}` shows indexing incomplete, the live LSP ops
(`get definition`, `get hover`, `get references`, `search workspace_symbol`) still
work immediately — do not wait on the index. If callgraph or blast radius comes
back empty for code that clearly compiles, the language server is missing or
warming up: check `{"op": "lsp status"}` and invoke `/lsp` if needed.

Fall back to raw Read/Grep/Glob only for non-code files (TOML, YAML, Markdown),
string literals and config values not in the symbol index, or confirming exact
syntax once code_context has already given you the location.

## Architecture Awareness

If an `ARCHITECTURE.md` file exists at the project root, read it before you act.
It is the project's own description of how the system is structured — its
modules and layers, the boundaries between them, and which direction
dependencies are allowed to flow. Treat it as authoritative context, the same
way you treat the code itself.

- **Orient with it.** Use it to place what you find — or what you build — inside
  the documented structure, instead of reconstructing the architecture from
  scratch by reading files.
- **Respect its boundaries.** Code should land in the module or layer the
  document assigns to it, and must not create dependency edges the document
  forbids (for example, a handler reaching past a service layer straight into
  storage).
- **Flag divergence.** If the work genuinely diverges from or extends the
  documented architecture — a new module, a new dependency direction, a new
  component — say so, and note that `ARCHITECTURE.md` needs an update to match.
  A stale architecture document is worse than none.

If no `ARCHITECTURE.md` exists, skip this — do not create one as a side effect.
The `/map` skill generates it deliberately when that is the goal.


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

Before editing anything, trace the failure with `code_context`: `get symbol` on the failing function or type to read it, `get callgraph` (inbound) to see what reaches it, and `get blastradius` on the file before you change it so the fix does not break a passing test elsewhere.

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
- When a fix adds new code or relocates existing code, place it per `ARCHITECTURE.md` if one exists — a fix that passes but violates a documented boundary is not done.
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
