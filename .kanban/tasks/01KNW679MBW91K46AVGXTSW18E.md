---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffb080
project: expr-filter
title: Move @lezer/highlight and @lezer/lr from devDependencies to dependencies
---
**File:** kanban-app/ui/package.json (lines 49-72)

**Severity:** warning

**What:** `@lezer/highlight` (^1.2.3) and `@lezer/lr` (^1.4.8) are declared under `devDependencies`, but they are imported by production source code that ships in the built bundle:

- `kanban-app/ui/src/lang-filter/parser.js` (line 2): `import {LRParser} from "@lezer/lr"`
- `kanban-app/ui/src/lang-filter/highlight.ts` (line 8): `import { styleTags, tags as t } from "@lezer/highlight";`

Both files are imported from runtime code (not tests or build scripts). The third package re-added by the same commit — `@codemirror/language` — was correctly placed in `dependencies`, so this is inconsistent within the same commit.

**Why it matters:**
1. The commit message explicitly claims "re-adds @lezer/lr, @lezer/highlight, and @codemirror/language as direct deps" — but only one is in `dependencies`. The intent and the implementation disagree.
2. For a `"private": true` Vite SPA, this will *work* (Vite bundles everything reachable from `src/`), so it is not a runtime blocker. However, it breaks any convention-following tooling: `eslint-plugin-import/no-extraneous-dependencies` with `devDependencies: false` for src files, `npm-check`, dep pruning, or future extraction of the UI into a publishable package will all treat these as missing production deps.
3. It makes future maintenance confusing — a reader looking up why `@lezer/highlight` is in `devDependencies` will conclude (wrongly) that it is build-time only.

**Suggestion:** Move both entries up into the `dependencies` block alongside `@codemirror/language`. Run `pnpm install` to regenerate `pnpm-lock.yaml` so the classification flip is reflected in the lockfile. `@lezer/generator` should stay in `devDependencies` — that one is genuinely a build-time codegen tool invoked by the `generate:grammar` script, not an import of shipped code.

**Subtasks:**
- [ ] Move `@lezer/highlight` from `devDependencies` to `dependencies` in kanban-app/ui/package.json
- [ ] Move `@lezer/lr` from `devDependencies` to `dependencies` in kanban-app/ui/package.json
- [ ] Run `pnpm install` from kanban-app/ui/ and commit the updated `pnpm-lock.yaml`
- [ ] Verification: `pnpm --filter swissarmyhammer-kanban-ui build` succeeds and `grep -A1 '"dependencies"' package.json` shows all three packages grouped together
#review-finding #expr-filter