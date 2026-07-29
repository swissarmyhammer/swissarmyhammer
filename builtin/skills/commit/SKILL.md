---
name: commit
description: Git commit workflow. Use this skill whenever the user says "commit", "save changes", "check in", or otherwise wants to commit code. Always use this skill instead of running git commands directly.
license: MIT OR Apache-2.0
compatibility: This skill requires `git` on the system PATH and a writable Git working tree.
agent: committer
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Commit

Create a git commit with a well-crafted conventional commit message.

## Guidelines

- Never commit scratch or temporary files. Commit only intentional source.
- Never miss files. You must commit all modified source on the branch.
- Create or update `.gitignore` for project-specific scratch patterns.
- **Kanban changes ship with code.** If `.kanban/` has modifications, always include them in the same commit. Never leave kanban state unstaged.

## Process

1. Review `git status`. Stage source and tests; skip scratch files.
2. Commit with a [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0/#summary) message.
3. Report progress.

## Examples

**Routine commit:** the user says "commit". `git status` shows `src/auth/login.rs`, `tests/auth.rs`, and untracked `scratch_notes.md`. Skip the scratch file. Stage the source and any `.kanban/` changes. Commit: `feat(auth): add JWT refresh endpoint`.

**Splitting unrelated work:** `git status` shows a bug fix in `src/parser.rs` and a docs tweak in `README.md`. Commit them separately: `fix(parser): handle empty input without panicking`, then `docs: clarify installation steps for macOS`.

## Troubleshooting

### `.kanban/` still shows modifications after committing

Kanban state was written after staging, or the stage step missed it. Amend the commit rather than create a follow-up:

```
git add .kanban
git commit --amend --no-edit
```

Use `git add -A` (or `git add . .kanban`) going forward.

### `git commit` fails with `pre-commit hook failed` / `husky > pre-commit`

A repo hook — husky, pre-commit, or lefthook — rejected the change. Read its output, fix the issue, re-stage, and retry. Never use `--no-verify` unless the hook itself is broken.

```
npx prettier --write .
git add -A
git commit -m "<same message>"
```

### Untracked scratch files keep appearing in `git status`

Add ignore patterns. Stage `.gitignore` in the same commit, if this is the first time:

```
echo 'scratch_*.md' >> .gitignore
echo '*.tmp' >> .gitignore
git add .gitignore
```
