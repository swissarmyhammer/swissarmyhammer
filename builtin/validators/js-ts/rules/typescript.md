---
name: typescript
description: unknown over any, no object/Function types, descriptive generics, readonly
severity: warn
---

# TypeScript

- **`unknown` over `any`.** `any` requires specific, documented justification.
- **No `object` or `Function` types.** Use `Record<string, unknown>` or `(...args: unknown[]) => unknown`.
- **No `I`-prefixed interfaces.** `Options`, not `IOptions`.
- **Descriptive generic names.** `Element`, `NewElement`, `InputType` — not `T`, `U`, `V`.
- **`readonly`** on properties and arrays not intended to be mutated, especially in return values and options interfaces.
- `number[]` not `Array<number>`. `readonly number[]` not `ReadonlyArray<number>`.
- **Test types with `tsd`.** Test files named `index.test-d.ts`, using `expectType<T>()`. Do not `await` in type tests — it accepts non-Promise values and renders the test meaningless.
