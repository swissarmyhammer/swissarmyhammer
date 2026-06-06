---
name: api-design
description: Default export for primary function, options objects, undefined over null
severity: warn
---

# JavaScript/TypeScript API Design

- **Default export for primary function.** `import pMap from 'p-map'`, not `import { pMap }`. Named exports for secondary utilities.
- **Options object for >2-3 parameters.** Enables optional fields, default values, forward-compatible extension.
- **`undefined` over `null`.** `typeof null === 'object'` is a JS design flaw. Default parameters only activate for `undefined`. The `no-null` rule enforces this.
- **`Uint8Array` over `Buffer`** for binary data. Buffer overrides Uint8Array methods inconsistently (notably `slice()` semantics differ).
- **Error messages must be descriptive.** Enough context to diagnose without reading source.
- **Immutable options.** Functions must not mutate the options object passed to them.
