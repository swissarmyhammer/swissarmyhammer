---
name: naming-and-style
description: No abbreviations, kebab-case filenames, for...of over forEach, no reduce
severity: warn
---

# JavaScript/TypeScript Naming and Style

- **No abbreviations.** `error` not `e`/`err`, `callback` not `cb`, `request` not `req`, `response` not `res`, `index` not `i`. Full words only.
- **Catch clauses use `error`**, not `e`, `err`, or `ex`.
- **Filenames are `kebab-case`.**
- **No nested ternaries.**
- **`for...of` over `.forEach()`.** forEach is not breakable, not awaitable, and harder to read.
- **`.find()` over `.filter()[0]`.**
- **`.at(-1)` over `[array.length - 1]`.**
- **No `Array#reduce`.** Use `map`, `filter`, or `for...of`. Reduce is almost always less readable.
- **`process.exitCode = 1`** (graceful) over `process.exit(1)` (abrupt), except in CLI entry points.
