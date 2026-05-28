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

- Never commit scratch or temporary files — only intentional source
- Never miss files: all modified source on the branch must be committed
- Create/update `.gitignore` for project-specific scratch patterns
- **Kanban changes ship with code**: if `.kanban/` has modifications, always include them in the same commit. Never leave kanban state unstaged.

## Process

1. **Detect project types** with `code_context` → `detect projects` to pick the right formatters/linters
2. **Format** for each detected type before staging (`cargo fmt`, `go fmt ./...`, `npx prettier --write .`)
3. **Lint** where applicable (`cargo clippy -- -D warnings`)
4. Review `git status` — stage source + tests, skip scratch
5. Commit with a [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0/#summary) message
6. Report progress

## Examples

**Routine commit:** User says "commit". Detect Rust → `cargo fmt` + `cargo clippy -- -D warnings`. `git status` shows `src/auth/login.rs`, `tests/auth.rs`, untracked `scratch_notes.md`. Skip scratch, stage source + any `.kanban/` changes, commit: `feat(auth): add JWT refresh endpoint`.

**Splitting unrelated work:** `git status` shows a bug fix in `src/parser.rs` and a docs tweak in `README.md`. Commit separately: `fix(parser): handle empty input without panicking`, then `docs: clarify installation steps for macOS`.

## Troubleshooting

### `.kanban/` still shows modifications after committing

Kanban state was written after staging or the stage step missed it. Amend rather than create a follow-up:

```
git add .kanban
git commit --amend --no-edit
```

Use `git add -A` (or `git add . .kanban`) going forward.

### `cargo clippy -- -D warnings` or `cargo fmt --check` fails pre-commit

Fix in place — never commit past failing lints. Don't `#[allow(...)]` to silence:

```
cargo fmt
cargo clippy --fix --allow-staged -- -D warnings
git add -A
```

Re-run `cargo clippy -- -D warnings` to confirm clean.

### `git commit` fails with `pre-commit hook failed` / `husky > pre-commit`

A repo hook (husky, pre-commit, lefthook) rejected the change. Read its output, fix the issue, re-stage, retry. Never use `--no-verify` unless the hook itself is broken.

```
npx prettier --write .
git add -A
git commit -m "<same message>"
```

### Untracked scratch files keep appearing in `git status`

Add ignore patterns and stage `.gitignore` in the same commit if first time:

```
echo 'scratch_*.md' >> .gitignore
echo '*.tmp' >> .gitignore
git add .gitignore
```
