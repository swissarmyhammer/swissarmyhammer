---
name: async-patterns
description: Promises over callbacks, async/await, bounded concurrency, top-level await
---

# JavaScript/TypeScript Async Patterns

- **Do not use callbacks.** Every async API must return a Promise.
- **Use `async`/`await`, not `.then()`/`.catch()` chains.** Exception: rare edge cases.
- **Do not wrap already-promise-returning code in `new Promise()`.**
- **Bound the concurrency.** Use `p-limit` or `p-map` with a concurrency option. Do not use `await` inside a `for` loop, unless you intend serial execution. Do not use unbounded `Promise.all()` on arrays that could grow large.
- **Do not pre-create promise arrays for `p-map`.** Pass a mapper function instead. A promise starts running as soon as you create it.
- **Use top-level `await` in ESM scripts.** This pattern is valid. This validator prefers it.
