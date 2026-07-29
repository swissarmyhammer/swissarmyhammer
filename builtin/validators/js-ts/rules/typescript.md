---
name: typescript
description: unknown over any, no object/Function types, descriptive generics, readonly
---

# TypeScript

- **Use `unknown`, not `any`.** Give a specific, documented reason before you use `any`.
- **Do not use `object` or `Function` types.** Use `Record<string, unknown>` instead of `object`. Use `(...args: unknown[]) => unknown` instead of `Function`.
- **Do not use `I`-prefixed interfaces.** Name the interface `Options`, not `IOptions`.
- **Use descriptive generic names.** Use names like `Element`, `NewElement`, or `InputType`. Do not use `T`, `U`, or `V`.
- **Add `readonly` to properties and arrays that must stay unchanged.** Do this most often in return values and options interfaces.
- **Use `number[]`, not `Array<number>`.** Use `readonly number[]`, not `ReadonlyArray<number>`.
- **Test types with `tsd`.** Name test files `index.test-d.ts`. Use `expectType<T>()` in these files. Do not use `await` in a type test. An `await` in a type test accepts non-Promise values. This makes the test meaningless.
