---
name: rust
description: Rust-specific duplication carve-outs — derive-style stubs and trait-impl forwarding one-liners
---

# Rust Duplication Carve-outs

Applies only to Rust files in scope. If no file in scope is Rust, this rule
contributes nothing.

Rust's trait system forces certain per-type boilerplate. These are not
duplication — do not flag them:

- **Derive-style stubs and simple trait impls per type** — a plain `Display`,
  `From`, `Deref`, or `Default` impl whose body is a single expression. The
  trait system requires one impl block per type; the repetition is the
  language's wiring, not copy-paste.
- **Trait-impl forwarding one-liners**: `impl Trait for T` methods whose body
  only delegates to an inherent method or an already-extracted shared helper.
  The shared logic is the helper; the impl block is the required dispatch
  wiring and cannot be merged across types.
- **Macro expansions**: code produced by `macro_rules!` or proc macros — if the
  expansion repeats, the macro is the single source; there is nothing to
  extract.

The test: does the repeated block contain logic that could drift out of sync?
A one-line delegation to a shared function cannot drift. Flag the copies only
when actual logic repeats — that is one function with an argument waiting to be
extracted.
