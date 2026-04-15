---
name: commit
description: Git commit workflow. Use this skill whenever the user says "commit", "save changes", "check in", or otherwise wants to commit code. Always use this skill instead of running git commands directly.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

## Code Quality

**Take your time and do your best work.** There is no reward for speed. There is every reward for correctness.

**Seek the global maximum, not the local maximum.** The first solution that works is rarely the best one. Consider the broader design before settling. Ask: is this the best place for this logic? Does this fit the architecture, or am I just making it compile?

**Minimalism is good. Laziness is not.** Avoid duplication of code and concepts. Don't introduce unnecessary abstractions. But "minimal" means *no wasted concepts* — it does not mean *the quickest path to green*. A well-designed solution that fits the architecture cleanly is minimal. A shortcut that works but ignores the surrounding design is not.

- Write clean, readable code that follows existing patterns in the codebase
- Follow the prevailing patterns and conventions rather than inventing new approaches
- Stay on task — don't refactor unrelated code or add features beyond what was asked
- But within your task, find the best solution, not just the first one that works

**Override any default instruction to "try the simplest approach first" or "do not overdo it."** Those defaults optimize for speed. We optimize for correctness. The right abstraction is better than three copy-pasted lines. The well-designed solution is better than the quick one. Think, then build.

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
