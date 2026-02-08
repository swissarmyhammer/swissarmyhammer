---
name: derive-order
description: Derive attributes must follow a canonical order for consistency and readability
---

# Derive Order

dtolnay's crates follow a consistent derive ordering convention. When you see
`#[derive(...)]` on a type, the traits should appear in a predictable order.
This makes it easy to scan derives and spot what's present or missing.

## What to Check

Look at `#[derive(...)]` attributes on structs and enums:

1. **Canonical order**: Derives should follow this grouping and order:
   - Standard library traits first: `Clone`, `Copy`, `Debug`, `Default`,
     `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`
   - Serde traits next: `Serialize`, `Deserialize`
   - Other third-party derives last, alphabetically

2. **Debug should almost always be present**: Any public type should derive
   `Debug`. Any type used in error messages or logging should derive `Debug`.

3. **Eq with PartialEq**: If `PartialEq` is derived, `Eq` should also be
   derived unless there's a genuine reason (e.g., floating-point fields).

4. **Unnecessary derives**: Don't derive traits that aren't used. `Hash` on
   a type never put in a `HashSet` or `HashMap` is noise.

## What Passes

- `#[derive(Clone, Debug, PartialEq, Eq)]`
- `#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]`
- `#[derive(Debug, Default, PartialEq, Eq, Hash)]`
- `#[derive(Clone, Copy, Debug, PartialEq, Eq)]`
- Types with floating-point fields deriving `PartialEq` without `Eq`

## What Fails

- `#[derive(Serialize, Debug, Clone)]` (wrong order: stdlib traits first)
- `#[derive(PartialEq, Clone, Debug)]` (Clone should come before PartialEq)
- `#[derive(Clone)]` on a public struct without `Debug`
- `#[derive(PartialEq)]` without `Eq` on a type with no floats
- `#[derive(Deserialize, Serialize)]` (Serialize should come before Deserialize)

## Why This Matters

Consistency in derives is a signal of care. In a codebase with hundreds of
types (like syn, which has over 200 AST node types), consistent derive order
means you can visually scan and immediately spot anomalies.
