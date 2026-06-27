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
severity: warn
---

# JavaScript/TypeScript Review Validator

Language-scoped review guidance migrated from the review skill's
`JS_TS_REVIEW.md` reference. These rules supplement the universal review
layers and apply to changed JavaScript/TypeScript (`.js`, `.jsx`, `.ts`,
`.tsx`) files only.

The glob set `**/*.js`, `**/*.jsx`, `**/*.ts`, `**/*.tsx` is the literal
equivalent of `**/*.{js,jsx,ts,tsx}`: the validator engine matches with the
`glob` crate, which does not expand `{a,b}` brace alternation, so the
extensions are listed individually.

Each rule is an **in-file idiom judgment** read from the diff — there are no
engine probes. Every rule that fires must be fixed — review is binary
pass/fail, with no advisory or severity tier among findings. Only add a rule to
this validator if you want it enforced; there are no advisory rules.
