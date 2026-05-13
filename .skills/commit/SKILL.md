---
name: commit
description: Git commit workflow. Use this skill whenever the user says "commit", "save changes", "check in", or otherwise wants to commit code. Always use this skill instead of running git commands directly.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for project detection to pick the right formatter/linter. Also requires `git` on the system PATH and a writable Git working tree.
metadata:
  author: swissarmyhammer
  version: 0.12.11
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

## Branching

- Work on the current branch unless instructed otherwise
- Don't create new branches without explicit request

## Commits

- Use conventional commit format: `type(scope): description`
- Types: feat, fix, refactor, test, docs, chore, style
- Write clear, concise commit messages explaining the "why"
- Don't commit scratch files, temporary outputs, or generated artifacts
- Ensure all relevant files are staged before committing

## Safety

- Never force push to main/master
- Don't amend commits that have been pushed
- Check git status before committing to avoid missing files


# Commit

Create a git commit with a well-crafted conventional commit message.


## Guidelines

- You MUST NOT commit scratch files that you generated, only commit source that you want in the project permanently
- You MUST NOT miss files on the commit
  - You MUST commit all the source files modified on the current branch
  - You MUST check for and create if needed a sensible project specific .gitignore
- **Kanban board**: If a `.kanban/` directory exists, ALWAYS include its changes in the same commit as the code. Task tracking lives with the code — tasks created, moved, or completed during this work must ship together. Never leave `.kanban/` changes unstaged.

## Process

- **Detect project types** using `code_context` → `detect projects` to identify formatters and linters
- **Run formatters** for each detected project type before staging (e.g., `cargo fmt` for Rust, `go fmt ./...` for Go, `npx prettier --write .` for Node.js)
- **Run linters** if the project has them (e.g., `cargo clippy -- -D warnings` for Rust)
- Evaluate the current `git status`, determine which files need to be added
- Clean up your scratch and temporary files
- Look for files that were modified and need to be part of the commit
- Look for files that were added and not yet staged, these need to be part of the commit unless they are one of your scratch files
- Commit your code with a [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0/#summary)
- Report your progress

## Examples

### Example 1: Routine commit after a feature change

User says: "commit"

Actions:
1. Run `code_context` `op: "detect projects"` to identify Rust — run `cargo fmt` and `cargo clippy -- -D warnings` before staging.
2. Evaluate `git status` — find `src/auth/login.rs` and `tests/auth.rs` modified, plus an untracked `scratch_notes.md`.
3. Skip the scratch file, stage the source and test changes, and include any modified files under `.kanban/` in the same commit.
4. Commit with a conventional message: `feat(auth): add JWT refresh endpoint`.

Result: One clean commit containing only intentional changes, formatted and linted, with kanban state shipped alongside the code.

### Example 2: Splitting unrelated work

User says: "check in my changes"

Actions:
1. `git status` shows two unrelated sets of changes: a bug fix in `src/parser.rs` and a docs tweak in `README.md`.
2. Stage and commit them separately rather than bundling.
3. First commit: `fix(parser): handle empty input without panicking`.
4. Second commit: `docs: clarify installation steps for macOS`.

Result: Two conventional commits, each with a single focused concern — readable history, easy to revert either independently.

## Troubleshooting

### Error: after committing, `git status` still shows modifications under `.kanban/`

- **Cause**: Kanban state (task moves, new tasks, comments) was written after the files were staged, or the stage step used a narrow glob that missed `.kanban/`. Task tracking lives with the code and must ship in the same commit.
- **Solution**: Amend the commit to pick up the kanban files, don't create a follow-up commit:
  ```
  git add .kanban
  git commit --amend --no-edit
  ```
  For future commits, stage with `git add -A` (or `git add . .kanban`) so kanban changes are never left behind.

### Error: `cargo clippy -- -D warnings` / `cargo fmt --check` fails during pre-commit

- **Cause**: Formatter or linter issues slipped in during the change. Committing as-is would land warnings in the history.
- **Solution**: Fix in place and re-stage — never commit past failing lints:
  ```
  cargo fmt
  cargo clippy --fix --allow-staged -- -D warnings
  git add -A
  ```
  Then re-run `cargo clippy -- -D warnings` to confirm clean before committing. Do not add `#[allow(...)]` to silence warnings.

### Error: `git commit` fails with `pre-commit hook failed` or `husky > pre-commit (node ...)`

- **Cause**: A repo-level pre-commit hook (e.g. `husky`, `pre-commit`, `lefthook`) ran a check — formatter, linter, or test — that rejected the staged changes. The commit object was never created.
- **Solution**: Read the hook's output (it prints the failing command), fix the underlying issue, re-stage, and retry. Never bypass with `--no-verify` unless the hook itself is broken. Example:
  ```
  npx prettier --write .
  git add -A
  git commit -m "<same message>"
  ```

### Error: untracked scratch files (e.g. `scratch_notes.md`, `tmp.log`) keep showing up in `git status`

- **Cause**: No `.gitignore` entry covers your scratch patterns, so every `git status` surfaces them and makes it easy to stage one accidentally.
- **Solution**: Add an ignore pattern and keep scratch out of the working tree going forward:
  ```
  echo 'scratch_*.md' >> .gitignore
  echo '*.tmp' >> .gitignore
  git add .gitignore
  ```
  Stage `.gitignore` in the same commit if this is the first time you add it.
