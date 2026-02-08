# dtolnay

Rust coding style rules inspired by [David Tolnay](https://github.com/dtolnay),
author of serde, syn, anyhow, thiserror, proc-macro2, and many other foundational
Rust crates.

These rules enforce the precise, minimal, type-driven style that dtolnay is known
for. They run as edit-time validators, checking every code change in real time.

## Rules

| Rule | What it enforces |
|------|-----------------|
| `parse-dont-validate` | Use parsing and type conversions instead of runtime validation checks |
| `meaningful-error-messages` | Error types and messages must help the user fix the problem |
| `no-stringly-typed` | Use enums and newtypes instead of raw strings for domain concepts |
| `exhaustive-matching` | Match arms must be exhaustive; no wildcard catch-alls hiding bugs |
| `precise-imports` | Import specific items, not glob imports |
| `no-unnecessary-allocation` | Don't allocate when a borrow or slice will do |
| `derive-order` | Derive attributes must follow a canonical order |
| `api-surface-minimality` | Public API should expose the minimum necessary surface |
| `lifetime-elision` | Don't write lifetime annotations that the compiler can infer |
| `no-type-complexity` | Flatten deeply nested generic types into named type aliases |
| `testing-as-documentation` | Tests are the spec: named behaviors, one assert per test, trybuild for compile errors |
| `formatting-discipline` | Run cargo fmt, then check import grouping, comment style, and what rustfmt misses |
| `test-coverage-intent` | Every public fn and every error path must have a test that exercises it |
| `test-isolation-state` | Tests touching files, env vars, or any stored state must be fully isolated |

## Installation

```bash
avp install dtolnay
```
