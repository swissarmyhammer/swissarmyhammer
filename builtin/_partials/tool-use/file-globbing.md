---
title: File Globbing Best Practices
description: How to use glob patterns effectively
partial: true
---

## File Globbing Best Practices

**CRITICAL: Avoid overly broad glob patterns.** Never use patterns like `*`, `**/*`, `*.*`, or `**/*.ext` (all files of an extension recursively) as they:
- Match thousands of files, causing performance issues and rate limiting
- Overflow context with excessive results
- Make output difficult to process

**Instead: Use specific, scoped patterns with directory constraints.**

When exploring a new codebase, use multiple small, targeted globs:

1. **Start with root config files** (one pattern per type):
   - `*.json` - package.json, tsconfig.json, etc.
   - `*.toml` - Cargo.toml, pyproject.toml, etc.
   - `*.yaml` or `*.yml` - CI configs, docker-compose
   - `*.lock` - lockfiles

2. **Then explore by specific directories** (never glob entire project for one extension):
   - JavaScript/TypeScript: `src/**/*.ts`, `src/**/*.tsx`, `test/**/*.test.js`
   - Rust: `src/**/*.rs`, `tests/**/*.rs`, `benches/**/*.rs`
   - Python: `src/**/*.py`, `tests/**/*.py`, `lib/**/*.py`
   - Go: `cmd/**/*.go`, `pkg/**/*.go`, `internal/**/*.go`

3. **Then specific subdirectories**:
   - `docs/**/*.md`
   - `.github/**/*.yml`
   - `scripts/**/*.sh`

**Good examples:**
- `src/**/*.rs` (scoped to src directory)
- `tests/**/*.py` (scoped to tests directory)
- `*.json` (only root level)

**Bad examples (NEVER use these):**
- `*` - matches everything in directory
- `**/*` - matches everything recursively
- `*.*` - matches all files with extensions
- `**/*.rs` - matches all Rust files everywhere (use `src/**/*.rs` instead)
- `**/*.py` - matches all Python files everywhere (use `src/**/*.py`, `tests/**/*.py` instead)
