# JavaScript/TypeScript Review Guidelines (Sindre Sorhus school)

Apply these when reviewing JavaScript or TypeScript code. These supplement the universal review layers.

## ESM-First

- `"type": "module"` in `package.json`. No exceptions.
- `"exports"` field, not `"main"`.
- All imports use full relative paths with explicit `.js` extensions: `import x from './utils.js'` — not `'./utils'` or `'.'`.
- No `require()`, no `module.exports`. Hard disqualifiers.
- No `'use strict'` — implicit in ESM.
- Built-in Node.js modules use the `node:` protocol prefix: `import fs from 'node:fs'` — not `'fs'`.
- Target Node.js 18+ in `"engines"`.

## TypeScript

- **`unknown` over `any`.** `any` requires specific, documented justification.
- **No `object` or `Function` types.** Use `Record<string, unknown>` or `(...args: unknown[]) => unknown`.
- **No `I`-prefixed interfaces.** `Options`, not `IOptions`.
- **Descriptive generic names.** `Element`, `NewElement`, `InputType` — not `T`, `U`, `V`.
- **`readonly`** on properties and arrays not intended to be mutated, especially in return values and options interfaces.
- `number[]` not `Array<number>`. `readonly number[]` not `ReadonlyArray<number>`.
- **Test types with `tsd`.** Test files named `index.test-d.ts`, using `expectType<T>()`. Do not `await` in type tests — it accepts non-Promise values and renders the test meaningless.

## Small Focused Modules

- A module does one thing, describable in a single sentence.
- If the README needs multiple `##` sections for different major behaviors, it may be two packages.
- **Composition over configuration.** Prefer smaller composable functions over one function with 12 options.
- Do not add features just because someone asked. If it belongs in a different module, say so.

## Async Patterns

- **No callbacks.** All async APIs return Promises.
- **`async`/`await` over `.then()`/`.catch()` chains.** Exception: rare edge cases.
- **No `new Promise()` wrapping** around already-promise-returning code.
- **Bounded concurrency.** Use `p-limit` or `p-map` with a concurrency option. No `await` in a `for` loop unless serial execution is intentional. No unbounded `Promise.all()` on arrays that could be large.
- Do not pre-create promise arrays for p-map. Pass a mapper function — promises are eager.
- **Top-level `await`** is valid and preferred in ESM scripts.

## API Design

- **Default export for primary function.** `import pMap from 'p-map'`, not `import { pMap }`. Named exports for secondary utilities.
- **Options object for >2-3 parameters.** Enables optional fields, default values, forward-compatible extension.
- **`undefined` over `null`.** `typeof null === 'object'` is a JS design flaw. Default parameters only activate for `undefined`. The `no-null` rule enforces this.
- **`Uint8Array` over `Buffer`** for binary data. Buffer overrides Uint8Array methods inconsistently (notably `slice()` semantics differ).
- **Error messages must be descriptive.** Enough context to diagnose without reading source.
- **Immutable options.** Functions must not mutate the options object passed to them.

## React Components

- **Named prop interfaces.** Every component gets a `interface FooProps` co-located above it. No anonymous inline object types (`}: { field: FieldDef; value: unknown; ... }`). Even for 2-prop components — the named interface is the documentation.
- **`Component` + `ComponentProps` naming convention.** `EntityCard` gets `EntityCardProps`, not `Props` or `IEntityCardProps`.
- **No hardcoded field/entity logic in components.** The UI is a metadata interpreter. Components dispatch on configured properties (`field.editor`, `field.display`, `field.icon`, `field.sort`) — never on `field.type.kind`, `field.name`, or `entity_type` string comparisons. If a component needs to know something about a field, that information must be a declared property on the field definition, not a hardcoded check.
- **No hardcoded entity type strings.** Don't write `entityType === "tag"` or `entity_type === "board"`. Entity-specific behavior belongs in entity/field definitions (YAML), not React components.
- **No hardcoded field name strings.** Don't write `getStr(entity, "name")` or `getStr(entity, "color")`. Use schema-declared properties like `mention_display_field`, `search_display_field`, or equivalent.
- **No `as Record<string, unknown>` casts on field types.** If you need a property from `field.type` (like `options`, `entity`, `derive`), it should be surfaced as a top-level field property or handled by the backend's `effective_*()` methods before reaching the frontend.

## Naming and Style

- **No abbreviations.** `error` not `e`/`err`, `callback` not `cb`, `request` not `req`, `response` not `res`, `index` not `i`. Full words only.
- **Catch clauses use `error`**, not `e`, `err`, or `ex`.
- **Filenames are `kebab-case`.**
- **No nested ternaries.**
- **`for...of` over `.forEach()`.** forEach is not breakable, not awaitable, and harder to read.
- **`.find()` over `.filter()[0]`.**
- **`.at(-1)` over `[array.length - 1]`.**
- **No `Array#reduce`.** Use `map`, `filter`, or `for...of`. Reduce is almost always less readable.
- **`process.exitCode = 1`** (graceful) over `process.exit(1)` (abrupt), except in CLI entry points.
