---
name: rust
description: Rust-specific duplication carve-outs for derive-style stubs and trait-impl forwarding one-liners
---

# Rust Duplication Carve-outs

This rule applies only to Rust files in scope. If no file in scope is Rust,
this rule contributes nothing.

Rust's trait system forces certain boilerplate per type. This boilerplate
is not duplication. Do not flag it:

- **Derive-style stubs and simple trait impls per type.** An example is a
  plain `Display`, `From`, `Deref`, or `Default` impl whose body is a
  single expression. The trait system requires one impl block per type.
  This repetition is the language's wiring. It is not copy-paste.
- **Trait-impl forwarding one-liners.** These are `impl Trait for T`
  methods whose body only delegates to an inherent method, or to an
  already-extracted shared helper. The shared logic lives in the helper.
  The impl block is the required dispatch wiring. You cannot merge the
  impl block across types.
- **Macro expansions.** This is code that `macro_rules!` or proc macros
  produce. If the expansion repeats, the macro is the single source. There
  is nothing to extract.

Apply this test: does the repeated block contain logic that could drift out
of sync? A one-line delegation to a shared function cannot drift. Flag the
copies only when actual logic repeats. This case is one function with an
argument, waiting to be extracted.
