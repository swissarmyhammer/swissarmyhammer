---
assignees:
- claude-code
position_column: todo
position_ordinal: f180
title: Investigate filter-editor.delete-scenario flake — first run after IDE/test cache fails, subsequent runs green
---
## Symptom

`kanban-app/ui/src/components/filter-editor.delete-scenario.test.tsx:172` — test `tag → append tag → delete to empty: saved filter clears` fails on the first cold run of `pnpm test` and passes on every subsequent run within the same Vitest browser session.

```
AssertionError: expected '#BL' to be ''
 ❯ src/components/filter-editor.delete-scenario.test.tsx:172:38
    172|     expect(view.state.doc.toString()).toBe("");
       |                                      ^
```

## Reproducer

1. From a fresh terminal, `cd kanban-app/ui && pnpm test` → 1 fail, 1984 pass, 1 skipped.
2. Run again: 0 fail, 1985 pass, 1 skipped. Stable green from then on.

In isolation `pnpm vitest run src/components/filter-editor.delete-scenario.test.tsx` is always green (7/7).

## Likely cause

Order-dependent CodeMirror state leaking from another filter-editor test in the suite. The `view.state.doc` reads `'#BL'` instead of `''` — strongly suggests a sibling test left state in a shared CodeMirror EditorView/store between runs, which the playwright-browser worker keeps warm after the first cold-cache run. Look for shared `EditorState` / `EditorView` setup helpers and ensure each test creates its own.

## Pre-existing

Test file last touched by commit `da7a9f591` ("fix(kanban-app): stabilize filter editor save loop and add diagnostics"), which is on `main`. Not introduced by spatial-nav step 1. #test-failure