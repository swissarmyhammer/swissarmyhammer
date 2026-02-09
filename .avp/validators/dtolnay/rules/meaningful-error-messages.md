---
name: meaningful-error-messages
description: Error types and messages must help the user understand and fix the problem
---

# Meaningful Error Messages

dtolnay's error handling philosophy (embodied in anyhow and thiserror) is that
errors exist to help the person reading them fix the problem. An error message
that says "invalid input" is a bug.

## What to Check

Examine error types, error messages, and error propagation in the changed code:

1. **Vague error strings**: Messages like "error", "failed", "invalid input",
   "something went wrong", or "unexpected value" without specifics.

2. **Missing context on propagation**: Using bare `?` to propagate errors
   through multiple layers without adding context via `.context()` or
   `.with_context()`.

3. **Error types that discard information**: Enum variants like
   `Error::Other(String)` that flatten structured errors into strings,
   losing the ability to match on them.

4. **Display impls that don't explain**: `impl Display for MyError` that
   produces messages without enough information to diagnose the issue.

5. **Swallowed errors**: Using `let _ = fallible_call()` or `.ok()` to
   silently discard errors without logging or documenting why.

## What Passes

- `return Err(anyhow!("config file {} not found at {}", name, path))`
- `.context("failed to read user preferences from disk")?`
- `.with_context(|| format!("failed to parse line {} of {}", lineno, filename))?`
- `#[error("expected {expected} but found {actual}")]` in thiserror derive
- `#[error("field `{field}` is required when `{condition}` is set")]`
- Intentionally discarded errors with a comment: `// best-effort cleanup, ok to fail`

## What Fails

- `return Err(anyhow!("invalid input"))`
- `return Err("error".into())`
- `.map_err(|_| "failed")?`
- `bail!("unexpected")` without saying what was unexpected or what was expected
- Bare `?` through 3+ function layers with no `.context()` anywhere in the chain
- `let _ = important_operation()` with no comment explaining why the error is ignored

## Why This Matters

dtolnay wrote anyhow specifically because Rust's error story was "you get a
type, but the message is useless." Good error messages are a feature, not
polish. They save hours of debugging.
