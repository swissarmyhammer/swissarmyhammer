---
name: comfy-table-listing
description: CLI listing output must use comfy-table for formatted display
---

# Comfy-Table Listing Rule

Any code that produces listing or tabular output for the CLI must use the `comfy-table` crate.

## What to Check

Look for code that displays lists, tables, or structured data to stdout in CLI commands:

- `println!` loops that format columnar output manually
- Hand-rolled ASCII table formatting with padding, alignment, or separators
- Using `format!` to build table rows with fixed-width fields
- Any tabular display that doesn't use `comfy_table::Table`

## What Passes

- Code using `comfy_table::Table` with `set_header` and `add_row`
- Non-tabular output (log messages, single-line status, error messages)
- Code that isn't CLI-facing (library internals, tests)

## What Fails

- Manual column formatting with spaces/tabs for alignment
- Building table-like output with `println!` and string padding
- Using any other table crate (prettytable, tabled, cli-table) instead of comfy-table
