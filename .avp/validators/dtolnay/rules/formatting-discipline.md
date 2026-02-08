---
name: formatting-discipline
description: Run cargo fmt to enforce formatting and maintain visual consistency in areas rustfmt does not cover
---

# Formatting Discipline

dtolnay runs rustfmt with default settings and never fights the formatter.
**After every edit to a `.rs` file, run `cargo fmt` on the affected crate.**
This is non-negotiable -- formatting is not a manual activity.

## Enforcement

**FIRST ACTION: Run `cargo fmt --check` on the crate containing the edited file.**

If it reports any formatting differences, the validation fails. Include the
diff in the failure message so the developer knows exactly what to fix.

After confirming `cargo fmt` compliance, check the items below that rustfmt
does not enforce.

## cargo fmt: What It Enforces

These are the default rustfmt behaviors that `cargo fmt` handles automatically.
Do not attempt to enforce these manually -- let the tool do it:

- **Indentation**: 4 spaces, no tabs
- **Max line width**: 100 characters, with automatic wrapping
- **Trailing commas**: Added automatically on multi-line constructs (structs, enums, function args, match arms)
- **Brace style**: `SameLineWhere` -- opening brace on the same line, dropping to next line after where clauses
- **Function signatures**: Automatic wrapping and alignment of parameters when they exceed line width
- **Match arms**: Consistent formatting of `=>` alignment and body wrapping
- **Closure formatting**: Automatic decisions on single-line vs multi-line closures
- **Chain formatting**: Method chains wrapped and indented consistently
- **Struct/enum layout**: Field alignment, trailing commas, consistent spacing
- **Use statement merging**: Combines `use` items from the same crate into grouped braces
- **Expression wrapping**: Binary expressions, if/else chains, and returns wrapped at line width

If a `rustfmt.toml` or `.rustfmt.toml` exists in the project, respect it.
If none exists, default rustfmt settings apply. Do not create one -- defaults
are the dtolnay way.

## What to Check (beyond cargo fmt)

rustfmt is intentionally conservative. It does not have opinions on the
following, so enforce them by inspection:

1. **Import grouping**: Imports should be separated into groups with a blank
   line between each:
   - `std` / `core` / `alloc` imports
   - External crate imports
   - `crate` / `self` / `super` imports

   rustfmt sorts within groups but does not insert blank lines between them.

2. **Comment style**: Use `//` line comments, not `/* */` block comments.
   Comments go above the code they describe, not at the end of the line
   (unless very short). Doc comments use `///` for items, `//!` for modules.
   rustfmt preserves comments as-is and does not restyle them.

3. **Attribute placement**: Each attribute on its own line above the item.
   Don't stack multiple attributes on one line. `#[derive(...)]` comes first,
   then `#[serde(...)]`, then other attributes. rustfmt does not reorder or
   split attributes.

4. **No `return` keyword at end of function**: The last expression in a
   function body should not use explicit `return`. The trailing expression
   *is* the return value. rustfmt does not remove unnecessary `return`
   keywords.

5. **Consistent string style**: Use raw strings `r#"..."#` for strings
   containing quotes or backslashes rather than escaping. rustfmt does not
   convert between string literal styles.

6. **Blank line discipline**: One blank line between functions. One blank
   line between logical sections within a function. No multiple consecutive
   blank lines. No blank line at the start or end of a block. rustfmt
   collapses some blank lines but not all of these cases.

## What Fails

- Any code where `cargo fmt` produces a diff (automatic failure, no exceptions)
- Import groups not separated by blank lines
- `return result;` as the last statement in a function body
- `/* block comment */` in normal code (acceptable in license headers)
- Multiple consecutive blank lines
- End-of-line comments explaining complex logic
- Multiple attributes stacked on a single line

## Why This Matters

dtolnay maintains crates read by thousands of developers daily. Formatting
is automated so humans never argue about it and PRs show only meaningful
changes. `cargo fmt` is the baseline. The manual checks above cover the
gaps where rustfmt has no opinion. Together they keep a large codebase
reviewable.
