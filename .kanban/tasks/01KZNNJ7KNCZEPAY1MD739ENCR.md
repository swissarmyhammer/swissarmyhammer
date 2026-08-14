---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m017aann0m29t39hz61568gq
  text: |-
    Picked up. Tool survey of `eslint-plugin-jsdoc` 63.3.3 `require-jsdoc` is done, and it answers all three carve-outs without a prompt-rule fallback.

    Root cause found for why a `contexts` selector on `MethodDefinition` reported nothing under `publicOnly: true`: the built-in `MethodDefinition` visitor calls `checkJsDoc` with `node.value` — the `FunctionExpression` — not the `MethodDefinition`. `publicOnly` runs `exportParser.isUncommentedExport`, whose `getExportAncestor` branch fails when the exported class itself carries a JSDoc block, and whose `isExportByAncestor` branch accepts the `FunctionExpression` and not the `MethodDefinition`. So a method selector must end in `> FunctionExpression`, which is the form the plugin's own option description gives. Measured: `MethodDefinition[kind="method"]` reports a method of an UNDOCUMENTED exported class and stays silent on the same method of a DOCUMENTED exported class; `MethodDefinition[kind="method"] > FunctionExpression` reports both.

    Second discovery: an abstract method is `TSAbstractMethodDefinition`, so the rule never read one and still does not. An overload signature, an optional `declared?()`, and a member of a `declare class` each carry `TSEmptyBodyFunctionExpression` rather than `FunctionExpression`, and the shipped rule reports all of them. A selector ending in `> FunctionExpression` alone would drop those four shapes, so each method selector ends in `> :matches(FunctionExpression, TSEmptyBodyFunctionExpression)`.
  timestamp: 2026-08-14T22:47:08.213098+00:00
- actor: claude-code
  id: 01m0183angvba7tjr0vjf2v16p
  text: |-
    Implementation landed, RED watched first.

    RED: the new passing fixture produced 7 findings under the shipped rule — `fixtures failed: the pass fixture missing-docs-typescript.pass.ts.tmpl produced 7 finding(s); none are allowed` — which took the rule out of the plan and failed the two new acceptance tests with it. GREEN after the rule change: all four TypeScript missing-docs tests pass.

    The three carve-outs, and what the tool actually offers for each:

    1. Simple accessors. `require-jsdoc` has first-class `checkGetters` and `checkSetters` options, both defaulting to `true`. They are REJECTED: each silences every accessor of its kind whatever the body holds, and the carve-out asks for a SIMPLE accessor. Two `contexts` selectors carry it instead: a getter reports when its body is not a single `return`, a setter when its body is not a single assignment. Corpus: 177 accessors before, 30 after; `checkGetters:false, checkSetters:false` would have left 0.
    2. Obvious implementations. No option exists. A `contexts` selector names `toString`, `valueOf`, `toLocaleString`, `toJSON` on an `Identifier` key and any method on a `Symbol.*` `MemberExpression` key. Corpus: 10 findings before, 0 after. `isExemptedImplementer` is the plugin's own neighbour and is far narrower — it needs a class that `implements` an interface whose member already carries JSDoc — so it answers none of these.
    3. Generated code. The `.d.ts` filter follows `dead-code-typescript` word for word rather than inventing a second reason. Corpus: 244 of 9341.

    Test carve-out: NOT left resting on an accident, and the card's stated cause is not the real one. `describe`/`it` being calls is not what holds it. The `ArrowFunctionExpression` visitor reads a function only when its parent is an assignment, a default export, a variable declarator, or a property, and the `FunctionExpression` visitor reads the same four plus everything when `checkAllFunctionExpressions` is on. Measured: the `function` callback form reports only with `publicOnly` off AND `checkAllFunctionExpressions: true` together; the arrow form answers to no setting at all. The config now writes `checkAllFunctionExpressions: false` so the one setting that can flip the `function` form is written down, and the passing fixture carries a `describe` holding one callback of each form, so the arrow form — which has no setting — is held by the fixture pair.

    Corpus: 6 repositories at named commits, 4306 `.ts`/`.tsx` files. 9498 findings before, 9097 after. The change is purely subtractive — 157 positions removed by the selectors, 244 more by the filter, and 0 positions in the new set that the old set did not hold.
  timestamp: 2026-08-14T23:00:47.408756+00:00
- actor: claude-code
  id: 01m018b944a6r57072ck4p057g
  text: |-
    One file beyond the card, and it is required rather than extra. `missing-docs.md` carries a measured survey of which of the six language rules keeps which carve-out, and two of its sentences named TypeScript on the wrong side: "Dart, Go, Python, Rust and TypeScript report [an undocumented public getter]" and "TypeScript reports an undocumented `toString()` and an undocumented `valueOf()`". This change makes both false. A stale measurement is worse than none, so both bullets now state the new reading and carry the corpus number.

    End-to-end check of the SHIPPED script, extracted from the rule's own `run:` block rather than a probe copy: over the 4306-file corpus it reports 9097, the number the rule body's last table states. A run given one `.d.ts` and nothing else reports nothing and exits 0. A run given no argument reports nothing and exits 0.

    ### implement — changed
    - evidence: 5 files — builtin/validators/code-hygiene/rules/missing-docs-typescript.md, builtin/validators/code-hygiene/rules/missing-docs.md, builtin/validators/code-hygiene/fixtures/missing-docs-typescript.pass.ts.tmpl, builtin/validators/code-hygiene/fixtures/missing-docs-typescript.fail.ts.tmpl, crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs. `cargo nextest run -p swissarmyhammer-validators` 738 passed, 0 failed. `cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-14T23:05:07.972784+00:00
position_column: doing
position_ordinal: '8280'
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