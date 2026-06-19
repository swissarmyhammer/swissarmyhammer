---
name: commit
description: Git commit workflow. Use this skill whenever the user says "commit", "save changes", "check in", or otherwise wants to commit code. Always use this skill instead of running git commands directly.
license: MIT OR Apache-2.0
compatibility: Requires `git` on the system PATH and a writable Git working tree.
agent: committer
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Commit

Create a git commit with a well-crafted conventional commit message.

## Guidelines

- Never commit scratch or temporary files — only intentional source
- Never miss files: all modified source on the branch must be committed
- Create/update `.gitignore` for project-specific scratch patterns
- **Kanban changes ship with code**: if `.kanban/` has modifications, always include them in the same commit. Never leave kanban state unstaged.

## Process

1. Review `git status` — stage source + tests, skip scratch
2. Commit with a [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0/#summary) message
3. Report progress

## Examples

**Routine commit:** User says "commit". `git status` shows `src/auth/login.rs`, `tests/auth.rs`, untracked `scratch_notes.md`. Skip scratch, stage source + any `.kanban/` changes, commit: `feat(auth): add JWT refresh endpoint`.

**Splitting unrelated work:** `git status` shows a bug fix in `src/parser.rs` and a docs tweak in `README.md`. Commit separately: `fix(parser): handle empty input without panicking`, then `docs: clarify installation steps for macOS`.

## Troubleshooting

### `.kanban/` still shows modifications after committing

Kanban state was written after staging or the stage step missed it. Amend rather than create a follow-up:

```
git add .kanban
git commit --amend --no-edit
```

Use `git add -A` (or `git add . .kanban`) going forward.

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
