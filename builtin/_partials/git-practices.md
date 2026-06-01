---
title: Git Practices
description: Git workflow guidelines
partial: true
---

## Branching

Work on the current branch. Don't create new branches without an explicit request.

## Commits

- Conventional format: `type(scope): description` (feat, fix, refactor, test, docs, chore, style)
- Messages explain the *why*
- Check `git status` before committing; never commit scratch, temp, or generated files

## Safety

Never force-push to main/master. Don't amend pushed commits.
