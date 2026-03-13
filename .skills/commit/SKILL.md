---
name: commit
description: Git commit workflow. Use this skill whenever the user says "commit", "save changes", "check in", or otherwise wants to commit code. Always use this skill instead of running git commands directly.
metadata:
  author: "swissarmyhammer"
  version: "1.0"
---

## Project Detection

To discover project types, build commands, and language-specific guidelines for this workspace, call the code_context tool:

```json
{"op": "detect projects"}
```

This will scan the directory tree and return:
- All detected project types (Rust, Node.js, Python, Go, Java, C#, CMake, Makefile, Flutter, PHP)
- Project locations as relative paths
- Workspace/monorepo membership
- Language-specific guidelines for testing, building, formatting, and linting

**Call this early in your session** to understand the project structure before making changes. The guidelines returned are authoritative — follow them for test commands, build commands, and formatting.

## Code Quality

- Write clean, readable code that follows existing patterns in the codebase
- Prefer simple, obvious solutions over clever ones
- Make minimal changes to achieve the goal - avoid unnecessary refactoring
- Don't add features, abstractions, or "improvements" beyond what was asked

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

## Process

- Evaluate the current `git status`, determine which files need to be added
- Clean up your scratch and temporary files
- Look for files that were modified and need to be part of the commit
- Look for files that were added and not yet staged, these need to be part of the commit unless they are one of your scratch files
- Commit your code with a [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0/#summary)
- Report your progress
