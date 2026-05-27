---
title: File Tools
description: Guidelines for file and command operations
partial: true
---

## File Tools

- **Paths:** always absolute — relative paths are not supported.
- **Commands:** use `shell` with `op: "execute command"`. Explain modifying commands before running.
- **Background:** append `&` for long-running processes (e.g. `node server.js &`).
- **Interactive:** avoid commands that prompt (e.g. `git rebase -i`); prefer non-interactive flags (`npm init -y`). Interactive shells hang until the user cancels.
