---
name: async-patterns
description: Promises over callbacks, async/await, bounded concurrency, top-level await
severity: warn
---

# JavaScript/TypeScript Async Patterns

- **No callbacks.** All async APIs return Promises.
- **`async`/`await` over `.then()`/`.catch()` chains.** Exception: rare edge cases.
- **No `new Promise()` wrapping** around already-promise-returning code.
- **Bounded concurrency.** Use `p-limit` or `p-map` with a concurrency option. No `await` in a `for` loop unless serial execution is intentional. No unbounded `Promise.all()` on arrays that could be large.
- Do not pre-create promise arrays for p-map. Pass a mapper function — promises are eager.
- **Top-level `await`** is valid and preferred in ESM scripts.
