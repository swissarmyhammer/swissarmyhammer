---
name: commit
description: Git commit workflow. Use this skill whenever the user says "commit", "save changes", "check in", or otherwise wants to commit code. Always use this skill instead of running git commands directly.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for project detection to pick the right formatter/linter. Also requires `git` on the system PATH and a writable Git working tree.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/coding-standards" %}
{% include "_partials/git-practices" %}

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
