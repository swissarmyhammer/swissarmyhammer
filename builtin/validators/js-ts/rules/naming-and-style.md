---
name: naming-and-style
description: No abbreviations, kebab-case filenames, for...of over forEach, no reduce
---

# JavaScript/TypeScript Naming and Style

- **Do not use abbreviations.** Use full words only. Use `error`, not `e` or `err`. Use `callback`, not `cb`. Use `request`, not `req`. Use `response`, not `res`. Use `index`, not `i`.
- **Use `error` in catch clauses.** Do not use `e`, `err`, or `ex`.
- **Name files in `kebab-case`.**
- **Do not nest ternary expressions.**
- **Use `for...of`, not `.forEach()`.** You cannot break out of `forEach`. You cannot use `await` inside `forEach`. `forEach` is harder to read.
- **Use `.find()`, not `.filter()[0]`.**
- **Use `.at(-1)`, not `[array.length - 1]`.**
- **Do not use `Array#reduce`.** Use `map`, `filter`, or `for...of` instead. `reduce` is almost always harder to read.
- **Set `process.exitCode = 1`, not `process.exit(1)`.** `process.exitCode = 1` shuts down the process gracefully. `process.exit(1)` stops the process abruptly. Exception: use `process.exit(1)` in CLI entry points.
