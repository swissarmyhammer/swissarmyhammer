---
name: api-design
description: Default export for primary function, options objects, undefined over null
severity: warn
---

# JavaScript/TypeScript API Design

- **Default export for primary function.** `import pMap from 'p-map'`, not `import { pMap }`. Named exports for secondary utilities.
- **Options object for >2-3 parameters.** Enables optional fields, default values, forward-compatible extension.
- **`undefined` over `null`** for absence you control. `typeof null === 'object'` is a JS design flaw; default parameters only activate for `undefined`. **Exception — never flag, and never suggest swapping `null`→`undefined`, when `null` is required by the type or contract:** a field declared `T | null`, a value deserialized from a backend/IPC/JSON/DB payload whose wire contract uses `null`, or any case where the change would not type-check (`tsc`). A suggestion that breaks compilation is a rule bug, not a finding — match the contract; correctness wins over stylistic preference.
- **`Uint8Array` over `Buffer`** for binary data. Buffer overrides Uint8Array methods inconsistently (notably `slice()` semantics differ).
- **Error messages must be descriptive.** Enough context to diagnose without reading source.
- **Immutable options.** Functions must not mutate the options object passed to them.
