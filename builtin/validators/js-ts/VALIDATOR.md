---
name: js-ts
description: >-
  JavaScript/TypeScript review guidelines (Sindre Sorhus school) — ESM-first,
  TypeScript types, small modules, async patterns, API design, React
  components, and naming/style idioms applied to changed JS/TS files.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.js"
    - "**/*.jsx"
    - "**/*.ts"
    - "**/*.tsx"
---

# JavaScript/TypeScript Review Validator

This file gives language-scoped review guidance. It comes from the review
skill's `JS_TS_REVIEW.md` reference. These rules add to the universal review
layers. They apply only to changed JavaScript and TypeScript files: `.js`,
`.jsx`, `.ts`, `.tsx`.

The glob set `**/*.js`, `**/*.jsx`, `**/*.ts`, `**/*.tsx` equals
`**/*.{js,jsx,ts,tsx}`. The validator engine matches files with the `glob`
crate. The `glob` crate does not expand `{a,b}` brace alternation. For this
reason, the file lists each extension by itself.

Each rule is an **in-file idiom judgment**. The reviewer reads the diff to
make this judgment. The engine runs no probes. You must fix every rule that
fires. The review result is pass or fail. There is no advisory tier or
severity tier among findings. Add a rule to this validator only when you want
the reviewer to enforce it. This validator has no advisory rules.
