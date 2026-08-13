---
assignees:
- claude-code
position_column: todo
position_ordinal: ffc680
title: missing-docs-typescript reports trivial getters and obvious methods, and filters no .d.ts
---
`builtin/validators/code-hygiene/rules/missing-docs-typescript.md` runs `jsdoc/require-jsdoc` at `publicOnly: true` over seven declaration kinds and three contexts, and declares `supersedes: [missing-docs]`.

Three carve-outs of `missing-docs.md` are dropped.

- "Simple getters/setters with self-explanatory names". A getter is a `MethodDefinition`, so `get name() { return this._name; }` on an exported class reports.
- "Obvious implementations (Display, Debug, ToString, etc.)". `MethodDefinition: true` requires JSDoc on `toString()` and on `[Symbol.iterator]()`. `jsdoc/require-jsdoc` has `exemptEmptyFunctions` but nothing that states "this method is an obvious interface implementation".
- "Generated code", and inconsistently so. The sibling `dead-code-typescript` drops `.d.ts` files, and names a generated lezer parser as the reason. This rule passes `.d.ts` and generated parser output straight through.

The prompt rule's closing note yields the obvious-implementation and getter carve-outs only to the Swift and Rust rules, so TypeScript does not hold that dispensation.

The private-item carve-out IS reproduced by `publicOnly: true`, and the test carve-out holds largely by accident: `describe` and `it` are call expressions, not declarations, so they fall outside the required node kinds.

Decide the `.d.ts` filter first — the sibling rule already states the reason.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity