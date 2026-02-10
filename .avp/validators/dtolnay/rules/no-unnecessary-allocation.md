---
name: no-unnecessary-allocation
description: Don't allocate when a borrow, slice, or Cow will do
---

# No Unnecessary Allocation

dtolnay's code is meticulous about avoiding needless allocations. If a function
only reads data, it should borrow. If it needs owned data, the caller should
decide when to allocate, not the callee.

## What to Check

Look for allocations that could be replaced by borrows or slices:

1. **`.to_string()` / `.to_owned()` for temporary use**: Creating an owned
   `String` just to pass it to a function that only needs `&str`.

2. **`String` parameters that should be `&str`**: Functions that take `String`
   but never store it, only read it.

3. **`.collect::<Vec<_>>()` before iteration**: Collecting an iterator into a
   `Vec` just to iterate over it again.

4. **`format!` for simple concatenation in hot paths**: Using `format!("{}{}", a, b)`
   when `write!` to a pre-allocated buffer or a simple push_str would suffice.

5. **`Vec<u8>` when `&[u8]` would work**: Allocating a vector of bytes for
   data that's only read, never mutated or stored.

6. **Cloning where borrowing works**: `.clone()` on data that could be
   borrowed with a lifetime adjustment.

7. **Missing `Cow` for conditional ownership**: Functions that always allocate
   a `String` when they sometimes return borrowed data and sometimes owned.
   `Cow<'_, str>` avoids the allocation on the borrow path.

8. **Rigid parameter types at API boundaries**: Taking `String` or `&str`
   when `impl AsRef<str>` or `impl Into<String>` would let callers pass
   whatever they already have without converting first. This pushes the
   allocation decision to the caller, who knows whether they already own.

9. **Returning `String` when `Cow` fits**: A function that returns its input
   unchanged in most cases but transforms it in edge cases. Returning
   `Cow<'_, str>` avoids allocating on the common path.

## What Passes

- Functions taking `&str` and `&[T]` for read-only access
- `impl AsRef<str>` for parameters that only need to read the string
- `impl Into<String>` for parameters that will be stored as owned `String`
- `Cow<'_, str>` for "maybe owned, maybe borrowed" return types
- `impl AsRef<Path>` for path parameters (the standard library pattern)
- Allocations that are genuinely needed (storing in a struct, sending across threads)
- `.to_string()` for error messages (error paths are not hot)
- `.collect::<Vec<_>>()` when you need random access or the length
- Clone on `Arc` (that's cheap, it's a reference count bump)

## What Fails

- `fn greet(name: String)` that only uses `name` in a `format!` call
- `let names: Vec<String> = iter.collect(); for name in &names { ... }`
  when `for name in iter { ... }` works
- `x.to_string()` passed immediately to a function taking `&str`
- `some_vec.clone()` when the function could take `&[T]`
- `format!("{}", single_variable)` instead of `single_variable.to_string()`
  (or just passing the variable directly)
- `fn normalize(s: &str) -> String` that returns the input unchanged 90% of
  the time -- should return `Cow<'_, str>`
- `fn open_file(path: &str)` instead of `fn open_file(path: impl AsRef<Path>)`
- `fn set_name(name: &str)` that immediately calls `name.to_string()` to store
  it -- should take `impl Into<String>` so callers with owned strings avoid
  the clone

## Why This Matters

serde processes data at massive scale. An unnecessary allocation per field
means millions of wasted allocations per document. dtolnay's zero-copy
deserialization in serde_json borrows directly from the input buffer wherever
possible. `Cow<'a, str>` is serde_json's secret weapon -- borrowed when the
input is clean, owned only when escape sequences force a new allocation. This
discipline extends to all code: let the caller choose when to allocate, use
`Cow` when ownership is conditional, and never copy data you only read.
