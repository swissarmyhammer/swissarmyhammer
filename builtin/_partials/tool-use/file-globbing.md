---
title: File Globbing Best Practices
description: How to use glob patterns effectively
partial: true
---

## File Globbing Best Practices

**Never use broad patterns** like `*`, `**/*`, `*.*`, or `**/*.ext`. They match thousands of files, overflow context, and trigger rate limits.

**Use scoped patterns with directory constraints.** When exploring, run multiple small globs:

1. **Root config first** (one per type): `*.json`, `*.toml`, `*.yaml`, `*.lock`
2. **Then by source directory** (never glob the whole project for one extension):
   - TS/JS: `src/**/*.ts`, `src/**/*.tsx`, `test/**/*.test.js`
   - Rust: `src/**/*.rs`, `tests/**/*.rs`
   - Python: `src/**/*.py`, `tests/**/*.py`
   - Go: `cmd/**/*.go`, `pkg/**/*.go`, `internal/**/*.go`
3. **Then subdirectories**: `docs/**/*.md`, `.github/**/*.yml`, `scripts/**/*.sh`

**Bad:** `*`, `**/*`, `*.*`, `**/*.rs`, `**/*.py` (unscoped).
**Good:** `src/**/*.rs`, `tests/**/*.py`, `*.json` (root only).
