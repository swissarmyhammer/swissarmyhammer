---
name: exhaustive-matching
description: Match arms must be exhaustive - no wildcard catch-alls that hide unhandled variants
---

# Exhaustive Matching

dtolnay's code reviews consistently push back on `_ =>` wildcard match arms.
When you match on an enum, list every variant. When a new variant is added,
the compiler tells you every place that needs updating. A wildcard silently
swallows new variants.

## What to Check

Look for match expressions and if-let chains on enums:

1. **Wildcard catch-all on owned enums**: `_ =>` or `_ => unreachable!()` in
   a match on an enum defined in this project. Every variant should be listed.

2. **Catch-all with identical behavior**: Multiple variants handled identically
   should be listed explicitly with `Variant1 | Variant2 => ...` instead of
   `_ => ...`.

3. **Wildcard on small enums**: Enums with 5 or fewer variants should never
   need a wildcard. List them all.

4. **`if let` hiding other variants**: Using `if let Some(x) = ...` when
   the `None` case should be handled explicitly, or when matching only one
   variant of a multi-variant enum.

## What Passes

- Explicit matching of all variants: `match color { Red => .., Green => .., Blue => .. }`
- Using `|` for shared behavior: `Red | Blue => "cool", Green | Yellow => "warm"`
- Wildcard on foreign enums (from external crates) that are `#[non_exhaustive]`
- Wildcard on primitive types where exhaustive listing is impractical (`match char_val { 'a'..='z' => .., _ => .. }`)
- `if let` when you genuinely only care about one variant and the else is a simple return/continue
- Wildcard on enums with many variants (>10) from external crates

## What Fails

- `match self.state { Active => handle(), _ => {} }` on a 3-variant enum
- `match direction { North => go_north(), _ => go_other() }` when there are only 4 directions
- Using `_ => unreachable!()` on a project-owned enum (list the variants and `unreachable!()` each if truly impossible)
- `match result { Ok(v) => v, Err(_) => default }` -- handle the error or use `.unwrap_or(default)`

## Why This Matters

In syn and serde, enums have dozens of variants and new ones are added across
versions. If match arms used wildcards, adding a new `Expr` variant would
silently do the wrong thing everywhere. Exhaustive matching is how dtolnay
keeps these massive crates correct as they evolve.
