---
name: precise-imports
description: Import specific items instead of using glob imports
---

# Precise Imports

dtolnay's crates import exactly what they use. Glob imports (`use foo::*`)
hide where things come from, create ambiguity when multiple globs export the
same name, and make it impossible to tell at a glance what a module depends on.

## What to Check

Examine `use` statements in the changed code:

1. **Glob imports from non-prelude modules**: `use std::collections::*`,
   `use crate::models::*`, `use some_crate::*`. Should list specific items.

2. **Overly broad module imports**: `use serde::*` instead of
   `use serde::{Deserialize, Serialize}`.

3. **Re-export globs in library code**: `pub use internal::*` in a lib.rs
   that should explicitly list its public API.

## What Passes

- `use std::collections::{HashMap, HashSet}`
- `use serde::{Deserialize, Serialize}`
- `use crate::error::{Error, Result}`
- Prelude imports: `use std::prelude::*` (that's what preludes are for)
- Test-only globs: `use super::*` inside `#[cfg(test)]` modules (common Rust testing pattern)
- Glob imports from trait extension modules explicitly designed for glob import
- `use std::fmt` (module import, not glob) followed by `fmt::Display` usage

## What Fails

- `use std::io::*` in production code
- `use crate::types::*`
- `pub use crate::internal::*` in a library's public API
- `use tokio::*`
- `use anyhow::*` (should be `use anyhow::{anyhow, bail, Context, Result}`)

## Why This Matters

In serde's codebase, you can look at any file's imports and immediately know
every external dependency. This makes refactoring safe -- you can see exactly
what breaks when you change an export. Glob imports turn this into guesswork.
