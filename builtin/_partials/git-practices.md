---
title: Git Practices
description: Git workflow guidelines
partial: true
---

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
