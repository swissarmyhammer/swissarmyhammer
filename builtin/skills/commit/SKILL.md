---
name: commit
description: Git commit workflow. Use this skill whenever the user says "commit", "save changes", "check in", or otherwise wants to commit code. Always use this skill instead of running git commands directly.
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
- **Kanban board**: If a `.kanban/` directory exists, ALWAYS include its changes in the same commit as the code. Task tracking lives with the code — cards created, moved, or completed during this work must ship together. Never leave `.kanban/` changes unstaged.

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
