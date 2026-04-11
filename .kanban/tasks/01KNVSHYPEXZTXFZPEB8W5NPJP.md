---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffac80
project: expr-filter
title: 'lang-filter: add Project token to Lezer grammar and highlight styling'
---
## What

Extend the CodeMirror 6 filter language so the filter editor recognises, highlights, and tokenises `$project-slug` as a first-class atom alongside `#tag` / `@user` / `^ref`.

**Files to modify (all in `kanban-app/ui/src/lang-filter/`):**

1. **`filter.grammar`** — Lezer grammar source (the current file is short; read it first):
   - Update the top comment to list `$project` alongside `#tag @mention ^ref`
   - Add a `Project` token inside `@tokens { ... }` mirroring the existing `Tag`, `Mention`, `Ref` definitions:
     ```
     Project { "$" $[\-a-zA-Z0-9_.]+ }
     ```
     Same body character class as the other atoms — this is slightly stricter than the chumsky parser in `swissarmyhammer-filter-expr` but the asymmetry already exists for Tag/Mention/Ref, so keep it consistent.
   - Add `Project` to the `@precedence { ... }` list at the bottom of the `@tokens` block. Current list is `{ Tag, Mention, Ref, AmpAmp, PipePipe, word }` — insert `Project` after `Ref`: `{ Tag, Mention, Ref, Project, AmpAmp, PipePipe, word }`
   - Update the `atom` rule to include `Project`: `atom { Tag | Mention | Ref | Project }`
   - Regenerate the Lezer parser output. Check `kanban-app/ui/package.json` for the exact build script (likely `pnpm run build:grammar` or a direct `lezer-generator` invocation). Based on commit `68021a262` touching these files, the generated `parser.js` and `parser.d.ts` ARE checked in — regenerate and commit them.

2. **`highlight.ts`** — **DO NOT add `Project` to `styleTags`.** Read the current file: `Tag` and `Mention` are intentionally omitted from `styleTags` because adding them caused `defaultHighlightStyle` to override the mention-decoration pill colors (see the inline comment in the file — commit `68021a262` fixed this). The file currently has:
   ```typescript
   export const highlighting = styleTags({
     // Tag and Mention are intentionally omitted — they get their visual styling
     // from the mention decoration system (colored pills), not syntax highlighting.
     Ref: t.link,
     "not and or": t.keyword,
     "Bang AmpAmp PipePipe": t.operator,
     "( )": t.paren,
   });
   ```
   Update the comment to mention `Project` as well (e.g. "Tag, Mention, and Project are intentionally omitted — they get their visual styling from the mention decoration system") so future readers know the omission is deliberate and uniform.

3. **`index.ts`** — read first. It exports the language support and may or may not need updates. Only modify if the export surface requires new token awareness.

4. **CSS pill color fallback** — grep for `--tag-color`, `--actor-color`, or `--task-color` under `kanban-app/ui/src/` (likely in a CSS module or a global stylesheet). Add a `--project-color` variable following the same pattern with a sensible neutral fallback color. The runtime color for a specific project comes from the project entity's `color` field via the `buildColorMap` path in `use-mention-extensions.ts`, but the CSS fallback must exist for when data hasn't loaded.

**Context:**
- The Lezer grammar is the editor-side parser. It runs on every keystroke for highlighting, decoration range detection, and editor validation. It must stay semantically aligned with the chumsky parser in `swissarmyhammer-filter-expr/src/parser.rs`: `$` as a project sigil accepting `[-a-zA-Z0-9_.]+` bodies.
- The two parsers do not share code. Alignment is enforced by tests that feed the same strings through both and compare acceptance.
- The decoration CSS class is auto-derived by `use-mention-extensions.ts::getDecoInfra` as `cm-${entityType}-pill`, so projects automatically get `cm-project-pill` once the mention infrastructure picks them up (handled in card `01KNVSK9V84X79TKGPP9XCYKFH`).

## Acceptance Criteria

- [ ] `filter.grammar` defines the `Project` token with the same body class as `Tag`/`Mention`/`Ref`
- [ ] `atom` rule includes `Project` alongside `Tag`, `Mention`, `Ref`
- [ ] The `@precedence` list inside `@tokens` includes `Project` (positioned after `Ref`, before `AmpAmp`)
- [ ] Top-of-file grammar comment mentions `$project`
- [ ] Generated `parser.js` / `parser.d.ts` are regenerated and committed
- [ ] `highlight.ts` does NOT add `Project` to `styleTags`; the inline comment is updated to include `Project` in the "intentionally omitted" list
- [ ] A `--project-color` CSS variable is defined with a neutral fallback
- [ ] Typing `$auth-migration` in the filter editor does NOT produce a parse error in the editor's inline validator
- [ ] Typing `$auth && #bug` parses cleanly
- [ ] Typing `$` alone surfaces a parse error (no crash)
- [ ] `cm-project-pill` class is visible on the rendered token after decoration wiring is complete (this verification requires card `01KNVSK9V84X79TKGPP9XCYKFH` to be merged — note this in the PR)

## Tests

Check `kanban-app/ui/src/lang-filter/` for existing grammar test files (e.g. `filter.grammar.test.ts` or similar). If tests exist:
- [ ] Add test cases asserting `parser.parse("$auth-migration")` produces a tree with a `Project` node at the expected position
- [ ] Add a test asserting a composite expression `$auth && #bug && @alice && ^01ABC` parses without errors and contains all four atom nodes
- [ ] Add a test asserting `$` alone is a parse error

If no tests exist, add a minimal `filter.grammar.test.ts` in the same folder that imports the generated parser and covers the cases above.

Test commands:
- [ ] `pnpm test` (or `npm test`) in `kanban-app/ui/` passes
- [ ] Manual verification in dev: open the filter editor, type `$somename`, confirm the text is tokenised as expected (no inline parse error). Full pill-decoration verification blocks on card `01KNVSK9V84X79TKGPP9XCYKFH`.

## Workflow
- Use `/tdd` if the Lezer test harness exists (write failing parser tests first). Otherwise, write tests alongside the grammar changes and ensure they fail before regenerating the parser. #expr-filter