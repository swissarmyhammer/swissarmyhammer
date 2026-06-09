---
name: error-handling
description: Typed errors for libraries, anyhow with context for applications
severity: error
---

# Rust Error Handling

**Library code uses typed errors; application code uses `anyhow`.**

- Libraries: return typed error enums via `thiserror`. Never return `anyhow::Error` or `Box<dyn Error>` from public APIs — callers lose the ability to match on specific failures.
- Applications: use `anyhow::Result<T>`. Every `?` on I/O or external calls must have `.context("what we were doing")`. A bare "No such file or directory" without context is a blocker.
- `Display` messages on errors: lowercase, no trailing punctuation.
- `Error::source()` chains must exist for wrapped errors — don't flatten the chain.
- Panics are for bugs only — internal invariant violations. Never panic on expected failure modes (bad input, missing files, network errors).
