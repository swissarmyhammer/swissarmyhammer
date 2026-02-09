---
name: no-allow-dead-code
description: Do not allow #[allow(dead_code)] - delete unused code instead
---

# No Allow Dead Code Rule

The `#[allow(dead_code)]` attribute must not be used. If code is unused, delete it. Version control preserves history.

## What to Check

Look for any occurrence of:

- `#[allow(dead_code)]`
- `#![allow(dead_code)]` (module-level)
- `#[cfg_attr(..., allow(dead_code))]`

## What Passes

- Code with no dead_code suppression attributes
- `#[allow(unused_imports)]` in test helper modules (this rule is specifically about dead_code)
- `#[cfg(test)]` modules (test code is allowed different lint rules)

## What Fails

- Any use of `#[allow(dead_code)]` on functions, structs, enums, traits, or modules
- Module-level `#![allow(dead_code)]`
- Using `dead_code` in any `allow` attribute outside of `#[cfg(test)]` modules
