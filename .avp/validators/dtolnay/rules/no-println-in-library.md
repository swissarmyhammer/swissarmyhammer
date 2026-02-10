---
name: no-println-in-library
description: Library code must not use println!/eprintln! - use tracing or return data instead
---

# No println! in Library Code

dtolnay's libraries never print to stdout or stderr. Output is a side effect
that belongs to the application, not the library. A library that prints steals
control from its caller -- the caller can't redirect, suppress, format, or
structure that output.

## What to Check

Look for print macros in non-binary, non-test code:

1. **`println!` in library code**: Library crates (`lib.rs` and its modules)
   must never write to stdout. Return data and let the caller decide how to
   display it.

2. **`eprintln!` for errors**: Use `tracing::error!` or return an `Err`.
   `eprintln!` goes to stderr unconditionally -- the caller can't intercept it.

3. **`print!` / `eprint!`**: Same as above, without the newline. Still
   unconditional side effects.

4. **`dbg!` left in code**: `dbg!` is for interactive debugging sessions. It
   prints to stderr and should never be committed.

5. **`println!` in CLI binary code for non-output purposes**: Even in binaries,
   status/progress messages should use `tracing::info!` so they respect
   verbosity settings. `println!` is only for the *primary output* of the
   command.

## What Passes

- `println!` in `main.rs` or `src/bin/*.rs` for primary command output
- `tracing::info!`, `tracing::warn!`, `tracing::error!` for diagnostic output
- `write!` / `writeln!` to an explicit `impl Write` parameter (the caller controls the destination)
- `log::info!` etc. if the project uses the `log` crate instead of `tracing`
- `println!` inside `#[cfg(test)]` modules (though `tracing` is still preferred)
- `eprintln!` in `main()` for fatal startup errors before tracing is initialized

## What Fails

- `println!` or `eprintln!` anywhere in a `lib.rs` module tree
- `dbg!` in committed code (any file)
- `print!` for progress indicators -- use a progress bar library or tracing
- `eprintln!("Warning: ...")` -- use `tracing::warn!`
- `println!` for debugging output that was never cleaned up

## Why This Matters

serde, syn, and anyhow are used in millions of projects. If serde printed
"deserializing field X" to stderr, every Rust binary using serde would have
noise on stderr. Libraries communicate through return values and types, not
through the terminal. dtolnay's crates have zero print statements in library
code.
