---
severity: error
tags:
- ui
- formatting
- tables
---

# Comfy-Table Formatting Rule

## Overview

When using the `comfy-table` crate for table rendering, **always use the `Cell` API with `.fg()` for colored text**. Never mix ANSI color codes (from crates like `colored` or `owo_colors`) directly in table cell content, as this breaks column alignment.

## Problem

Comfy-table cannot correctly calculate column widths when ANSI escape codes are embedded in strings. This causes misaligned columns:

```rust
// ❌ WRONG - Breaks alignment
let agent_name = "(default)".dimmed().to_string();
table.add_row(vec![use_case.to_string(), agent_name, source]);
```

The table will look broken:
```
│ Use Case  ┆ Agent             ┆ Source          │
│ root      ┆ (default) ┆ default │  // Misaligned!
```

## Solution

Use `Cell::new()` with `.fg()` for all colored cells:

```rust
// ✓ CORRECT - Proper alignment
let agent_cell = Cell::new("(default)").fg(Color::DarkGrey);
table.add_row(vec![
    Cell::new(use_case.to_string()),
    agent_cell,
    Cell::new(source),
]);
```

## Required Imports

```rust
use comfy_table::{Cell, Color, Table};
```

## Color Mapping

Map from external color crates to comfy-table colors:

| colored/owo_colors | comfy-table equivalent |
|-------------------|------------------------|
| `.green()` | `.fg(Color::Green)` |
| `.red()` | `.fg(Color::Red)` |
| `.yellow()` | `.fg(Color::Yellow)` |
| `.dimmed()` / `.bright_black()` | `.fg(Color::DarkGrey)` |
| `.blue()` | `.fg(Color::Blue)` |
| `.cyan()` | `.fg(Color::Cyan)` |

## Check Pattern

Look for any of these anti-patterns in code that uses `comfy-table`:

1. **Using `.to_string()` after color methods**:
   ```rust
   // ❌ BAD
   let text = "value".red().to_string();
   table.add_row(vec![text]);
   ```

2. **Using `format!()` with colored text**:
   ```rust
   // ❌ BAD
   let text = format!("{}", "value".green());
   table.add_row(vec![text]);
   ```

3. **Mixing plain strings and colored strings**:
   ```rust
   // ❌ BAD - Inconsistent
   table.add_row(vec![
       "plain text",
       "colored".green().to_string(),
   ]);
   ```

4. **Not wrapping ALL cells in `Cell::new()`**:
   ```rust
   // ❌ BAD - Inconsistent cell types
   table.add_row(vec![
       "plain string",  // String
       Cell::new("colored").fg(Color::Red),  // Cell
   ]);
   ```

## Correct Pattern

```rust
use comfy_table::{Cell, Color, Table};

let mut table = Table::new();

// ✓ CORRECT - All cells wrapped consistently
table.add_row(vec![
    Cell::new("Status"),
    Cell::new("✓").fg(Color::Green),
    Cell::new("Success"),
]);

table.add_row(vec![
    Cell::new("Error"),
    Cell::new("✗").fg(Color::Red),
    Cell::new("Failed"),
]);

table.add_row(vec![
    Cell::new("Warning"),
    Cell::new("⚠").fg(Color::Yellow),
    Cell::new("Caution"),
]);
```

## Why This Matters

1. **Alignment**: Comfy-table needs to know the true width of text to calculate column widths
2. **Consistency**: Using the same API throughout ensures predictable rendering
3. **Cross-platform**: Comfy-table handles terminal capabilities correctly
4. **Performance**: Comfy-table can optimize rendering when it controls all formatting

## Enforcement

Check that:
- All table cells use `Cell::new()` when colors are involved
- No `.dimmed()`, `.red()`, `.green()`, etc. followed by `.to_string()` in table row data
- Imports include `Cell` and `Color` from `comfy_table`
- No direct ANSI escape codes in strings passed to tables

## Exceptions

If you need colored text **outside** of tables, use `colored` or `owo_colors` freely:
```rust
// ✓ CORRECT - Not in a table
println!("{} Success!", "✓".green());
eprintln!("{} Error occurred", "✗".red());
```
