---
name: api-design
description: Default export for primary function, options objects, undefined over null
---

# JavaScript/TypeScript API Design

- **Export the primary function as the default export.** Write `import pMap from 'p-map'`, not `import { pMap }`. Export secondary utilities as named exports.
- **Use an options object when a function takes more than 2 or 3 parameters.** An options object allows optional fields. An options object allows default values. An options object allows forward-compatible extension.
- **Use `undefined`, not `null`, for an absence you control.** `typeof null === 'object'` is a flaw in JS design. A default parameter activates only for `undefined`, not for `null`.
  **Exception:** You must not flag `null`, and you must not suggest a change from `null` to `undefined`, in these cases:
  - The type declares the field as `T | null`.
  - The value comes from a backend, IPC, JSON, or database payload, and the wire contract uses `null`.
  - The change would break type-checking (`tsc`).

  A suggestion that breaks compilation is a rule bug, not a finding. Match the contract. Correctness wins over style preference.
- **Use `Uint8Array`, not `Buffer`, for binary data.** `Buffer` overrides `Uint8Array` methods inconsistently. For example, `slice()` behaves differently in each.
- **Write descriptive error messages.** Give enough context to diagnose the error without reading the source code.
- **Do not mutate options.** A function must not change the options object passed to it.
