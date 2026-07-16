---
name: error-handling
description: Typed errors for libraries, anyhow with context for applications
---

# Rust Error Handling

**Library code uses typed errors; application code uses `anyhow`.**

Classify by target, not by folder: a crate with only binary targets is an
application; a crate whose lib target is consumed by other crates (a workspace
member other crates depend on, or a published library) is a library. When a
crate is both, its public API follows the library rule and `anyhow` stays in
the bin entry points. This classification is the tiebreaker — converting a
crate's errors back and forth between `thiserror` and `anyhow` across review
rounds is always a validator error.

- Libraries: return typed error enums via `thiserror`. Never return `anyhow::Error` or `Box<dyn Error>` from public APIs — callers lose the ability to match on specific failures.
- Applications: use `anyhow::Result<T>`. Every `?` on I/O or external calls must have `.context("what we were doing")`. A bare "No such file or directory" without context is a blocker.
- `Display` messages on errors: lowercase, no trailing punctuation.
- `Error::source()` chains must exist for wrapped errors — don't flatten the chain.
- Panics are for bugs only — internal invariant violations. Never panic on expected failure modes (bad input, missing files, network errors).
