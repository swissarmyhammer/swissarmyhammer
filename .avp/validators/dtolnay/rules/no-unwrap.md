---
name: no-unwrap
description: Don't use unwrap() or expect() in production code - handle errors properly
---

# No Unwrap in Production Code

dtolnay's libraries never panic on recoverable errors. `unwrap()` and `expect()`
are time bombs: they work until they don't, and when they blow up in production,
they take down the whole process with a useless backtrace instead of a
meaningful error message.

## What to Check

Look for `.unwrap()` and `.expect(...)` calls in non-test code:

1. **`.unwrap()` on `Result`**: Almost always wrong in production code. Use `?`
   to propagate, `.context()` to add information, or match on the error.

2. **`.unwrap()` on `Option`**: Use `?` with `.ok_or()` / `.ok_or_else()`,
   or use `if let` / `match` to handle `None`.

3. **`.expect("...")` as documentation**: `expect` with a message is marginally
   better than `unwrap`, but still panics. If you can explain *why* it won't
   fail in the expect message, you can probably restructure the code so the
   compiler proves it won't fail.

4. **Unwrap in `main()` or CLI entry points**: Use `anyhow::Result` as the
   return type of `main()` instead.

5. **Chained unwraps**: `foo.unwrap().bar().unwrap()` -- two panic sites in
   one expression. Restructure with `?` and combinators.

## What Passes

- `.unwrap()` inside `#[cfg(test)]` modules (tests should panic on unexpected failures)
- `.unwrap()` on values that are *provably* infallible: `"123".parse::<i32>().unwrap()`
  where the input is a literal. Even then, prefer `const` or a comment.
- `.expect("hardcoded regex is valid")` on `Regex::new` with a literal pattern --
  but only at initialization, not in hot paths.
- `unreachable!()` after a match arm that the type system can't rule out
- Using `?` with `.context()` or `.with_context()` for error propagation

## What Fails

- `.unwrap()` on any `Result` from I/O, parsing, or network operations
- `.unwrap()` on `Option` from `.get()`, `.first()`, `.last()`, `.find()`
- `.expect("should not fail")` -- if you can't explain *why* it won't fail,
  it can fail
- `.unwrap()` inside library code (crates consumed by other crates)
- `.unwrap()` on user-supplied or runtime data
- `lock().unwrap()` on a `Mutex` -- use `lock().expect("mutex poisoned")` at
  minimum, or better, handle poisoning

## Why This Matters

anyhow exists because dtolnay saw too much Rust code panicking where it should
have returned errors. serde never panics on malformed input -- it returns
`Err`. syn never panics on invalid syntax -- it returns `Err`. A library that
panics is a library that can't be used in production.
