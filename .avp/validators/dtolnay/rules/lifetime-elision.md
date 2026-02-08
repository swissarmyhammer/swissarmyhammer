---
name: lifetime-elision
description: Don't write lifetime annotations that the compiler can infer
---

# Lifetime Elision

Rust's lifetime elision rules exist for a reason: they handle the common cases
so you don't have to write noisy annotations. dtolnay's code never writes a
lifetime that the compiler would infer. Unnecessary lifetimes add visual
complexity without adding information.

## What to Check

Look for explicit lifetime annotations that are redundant:

1. **Single-reference parameter functions**: Functions with one reference
   parameter don't need explicit lifetimes. The output lifetime is inferred
   from the single input.
   - `fn first_word<'a>(s: &'a str) -> &'a str` should be `fn first_word(s: &str) -> &str`

2. **Method signatures with `&self`**: Methods taking `&self` or `&mut self`
   get the output lifetime from `self` automatically.
   - `fn name<'a>(&'a self) -> &'a str` should be `fn name(&self) -> &str`

3. **`'_` where inference works**: Using `'_` in positions where the compiler
   would infer the same lifetime without any annotation.

4. **Unnecessary lifetime parameters on structs**: Adding a lifetime parameter
   to a struct when the struct could own its data instead.

## What Passes

- `fn parse(input: &str) -> &str` (elision handles this)
- `fn name(&self) -> &str` (elision from &self)
- Explicit lifetimes when there are multiple reference inputs and the
  compiler can't infer which one the output borrows from
- Lifetimes on structs that genuinely borrow data: `struct Tokenizer<'a> { input: &'a str }`
- `'static` lifetimes (these are never elidable)
- `'_` in turbofish or type positions where it improves clarity

## What Fails

- `fn len<'a>(s: &'a str) -> usize` (no reference in output, lifetime is unused)
- `fn first<'a>(s: &'a [u8]) -> &'a u8` (single input reference, elision works)
- `fn get<'a>(&'a self) -> &'a str` (method with &self, elision works)
- `impl<'a> Display for Wrapper<'a>` when `Wrapper` doesn't need to borrow
- Lifetime parameters that only appear once in the function signature

## Why This Matters

syn and proc-macro2 process complex token trees with many borrowing
relationships. When lifetimes are used, they're meaningful -- they tell you
something the compiler can't figure out alone. If every function had
redundant lifetime noise, the ones that actually matter would be invisible.
