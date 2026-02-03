---
title: File Tools
description: Guidelines for file and command operations
partial: true
---

## File Tools

- **File Paths:** Always use absolute paths when referring to files with tools. Relative paths are not supported. You must provide an absolute path.
- **Command Execution:** Use the 'shell_execute' tool for running shell commands, remembering the safety rule to explain modifying commands first.
- **Background Processes:** Use background processes (via `&`) for commands that are unlikely to stop on their own, e.g. `node server.js &`.
- **Interactive Commands:** Try to avoid shell commands that are likely to require user interaction (e.g. `git rebase -i`). Use non-interactive versions of commands (e.g. `npm init -y` instead of `npm init`) when available, and otherwise remind the user that interactive shell commands are not supported and may cause hangs until canceled by the user.
