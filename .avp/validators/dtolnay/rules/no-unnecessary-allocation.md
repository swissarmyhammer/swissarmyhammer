---
name: no-unnecessary-allocation
description: Don't allocate when a borrow or slice will do
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

## What Passes

- Functions taking `&str` and `&[T]` for read-only access
- Using `impl AsRef<str>` or `impl Into<String>` for flexible API boundaries
- `Cow<'_, str>` for "maybe owned, maybe borrowed" scenarios
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

## Why This Matters

serde processes data at massive scale. An unnecessary allocation per field
means millions of wasted allocations per document. dtolnay's zero-copy
deserialization in serde_json borrows directly from the input buffer wherever
possible. This discipline extends to all code.
