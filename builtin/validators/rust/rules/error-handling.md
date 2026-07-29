---
name: error-handling
description: Use typed errors in libraries, and anyhow with context in applications
---

# Rust Error Handling

**Use typed errors in library code. Use `anyhow` in application code.**

Classify each crate by its target, not by its folder. A crate with only
binary targets is an application. A crate whose lib target other crates
depend on is a library. This includes a workspace member that other crates
depend on, and a published library. When a crate is both, its public API
must follow the library rule. In this case, `anyhow` stays only in the bin
entry points.

This classification is the tiebreaker. Do not convert a crate's errors back
and forth between `thiserror` and `anyhow` across review rounds. This
conversion is always a validator error.

- In libraries, return typed error enums through `thiserror`. Never return `anyhow::Error` or `Box<dyn Error>` from a public API. Callers lose the ability to match on specific failures.
- In applications, use `anyhow::Result<T>`. Add `.context("what we were doing")` to every `?` on an I/O call or an external call. A bare message like "No such file or directory" without context is a blocker.
- Write `Display` messages on errors in lowercase, with no trailing punctuation.
- Keep the `Error::source()` chain for wrapped errors. Do not flatten the chain.
- Use panics only for bugs, meaning internal invariant violations. Never panic on an expected failure mode, for example bad input, a missing file, or a network error.
