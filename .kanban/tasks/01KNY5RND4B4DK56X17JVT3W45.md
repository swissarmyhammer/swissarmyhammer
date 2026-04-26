---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff8880
project: expr-filter
title: 'Wire $project sigil into filter editor UX: placeholder, filter-sigil autocomplete, integration and scenario tests'
---
## What

The `$project` atom is already supported by the DSL at the parser level — both the Rust chumsky parser (`swissarmyhammer-filter-expr/src/parser.rs`) and the Lezer grammar (`kanban-app/ui/src/lang-filter/filter.grammar`) accept `$slug` and produce `Expr::Project` / `Project` parse nodes. Unit tests for each layer already exist (`parser::tests::project_*`, `filter grammar parser > parses a project atom`, `task::list::test_list_tasks_filter_by_project*`).

What's missing is the end-to-end user-facing wiring and test coverage for the perspective filter editor:

1. **Placeholder doesn't advertise `$project`.** `kanban-app/ui/src/components/filter-editor.tsx` passes `placeholder="Filter… e.g. #bug @alice"` to the `TextEditor`. Only `#` and `@` are shown. A user who opens the filter bar has no hint that `^ref` or `$project` are valid sigils.

2. **`includeFilterSigils` block omits `$`.** `kanban-app/ui/src/hooks/use-mention-extensions.ts` in `buildMentionExtensions`, when `includeFilterSigils` is true, hardcodes fallback completion sources for `@` (actor) and `^` (task) but **not** `$` (project). The schema-driven loop above only registers a completion source if `colorMap.size > 0`, so when zero projects are loaded into the entity store the project autocomplete silently disappears — even though the grammar accepts the atom. Add a matching hardcoded `createMentionCompletionSource("$", buildAsyncSearch("project"))` call so project autocomplete works in the filter editor regardless of entity-store state. `search_mentions` (kanban-app/src/commands.rs) already accepts `entity_type: "project"` generically, so no backend work is needed.

3. **No end-to-end integration test for `$project` through dispatch.** `swissarmyhammer-kanban/tests/filter_integration.rs` has 19 scenarios (`s01`–`s19`) covering tags, assignees, refs, virtual tags, grouping, boolean logic, edge cases and one perspective round-trip — but none exercise `$project`. The existing `test_list_tasks_filter_by_project*` tests in `src/task/list.rs` go through the direct operation API, not the dispatch path (`parse_input` → `execute_operation`), so they do not catch regressions in JSON parsing, dispatch routing, or filter-plumbing for projects.

4. **No frontend scenario test for `$project` in the filter editor.** `kanban-app/ui/src/components/filter-editor.test.tsx` has scenarios for `#bug`, `#bug && @will`, `#paper #READY`, and empty input — but none type a `$project` expression and verify it dispatches cleanly (and doesn't show the `Invalid filter expression` error state).

### Approach

Follow the existing patterns in each file — no new abstractions.

**Placeholder text** — change the `FilterEditor`'s placeholder to showcase the three most common sigils plus `$project`. Keep it short. Suggested: `"Filter… e.g. #bug @alice $spatial-nav"`. The `TextEditor` forwards the `placeholder` prop to `StableCodeMirror`, which renders it via CM6's built-in placeholder extension as `.cm-placeholder` in the DOM — assertable with `container.querySelector(".cm-placeholder")`.

**Filter-sigil hardcoding** — in `use-mention-extensions.ts`'s `buildMentionExtensions`, add a third `completionSources.push(...)` call under the existing `if (includeFilterSigils)` block:

```ts
completionSources.push(
  createMentionCompletionSource("$", buildAsyncSearch("project")),
);
```

This mirrors the existing `@` and `^` fallbacks and is a one-line addition in the same block.

**Rust integration scenarios** — add three scenarios at the end of `swissarmyhammer-kanban/tests/filter_integration.rs`, following the `setup()` / `dispatch()` / `titles()` helpers already in place:

- `s20_filter_by_project`: add a project via `{"op": "add project", "id": "auth", "name": "Auth"}`, add two tasks (one with `project: "auth"`, one without), dispatch `{"op": "list tasks", "filter": "$auth"}`, assert only the scoped task is returned.
- `s21_project_combined_with_tag`: same setup plus a tagged task, dispatch `"$auth && #bug"`, assert exactly one match.
- `s22_perspective_roundtrip_with_project_filter`: add a perspective with `"filter": "$auth"`, get it, assert the filter is preserved; update to `"$auth || #urgent"`, re-read, assert persisted. Mirrors `s19` but with the project sigil.

**Frontend scenario tests** — add two tests to `filter-editor.test.tsx`:

- `placeholder includes $project example`: render `<FilterEditor filter="" perspectiveId="p1" />`, assert `container.querySelector(".cm-placeholder")?.textContent` contains `$`.
- `dispatches perspective.filter for $project expression`: type `$spatial-nav` into the CM6 editor via the existing `getEditorView(container)` helper, wait the debounce, assert `mockInvoke` was called with `cmd: "perspective.filter"` and `args: { filter: "$spatial-nav", perspective_id: "p1" }`. Do NOT assert autocomplete popup behavior — that's a separate layer covered by `use-mention-extensions` tests.

### Subtasks

- [x] Update placeholder in `kanban-app/ui/src/components/filter-editor.tsx` to `"Filter… e.g. #bug @alice $spatial-nav"`.
- [x] In `kanban-app/ui/src/hooks/use-mention-extensions.ts` `buildMentionExtensions`, add `createMentionCompletionSource("$", buildAsyncSearch("project"))` to the `includeFilterSigils` block alongside `@` and `^`.
- [x] Add integration scenarios `s20`, `s21`, `s22` to `swissarmyhammer-kanban/tests/filter_integration.rs` covering project filter, project+tag combination, and perspective round-trip with `$project`.
- [x] Add frontend scenario test to `kanban-app/ui/src/components/filter-editor.test.tsx` asserting the rendered placeholder contains a `$` hint.
- [x] Add frontend scenario test to `kanban-app/ui/src/components/filter-editor.test.tsx` asserting typing `$spatial-nav` dispatches `perspective.filter` (not an error state).

## Acceptance Criteria

- [x] Opening the perspective filter bar with an empty filter shows placeholder text that includes a `$`-prefixed example (visible in the rendered CM6 placeholder span).
- [x] Typing `$spatial-nav` into the filter editor produces a valid parse (no `Invalid filter expression` styling) and, after the 300 ms autosave debounce, dispatches `perspective.filter` with the literal text.
- [x] After the prior card 01KNWQ34B7864MV7MEX0YE4KGJ lands loading projects into `entitiesByType`, typing `$s` in the filter editor opens the autocomplete popup with project matches. Absent that fix this card still works because `use-mention-extensions.ts` now hardcodes `$` completion alongside `@` and `^`.
- [x] `cargo test -p swissarmyhammer-kanban --test filter_integration` runs the new `s20`–`s22` scenarios and they pass.
- [x] `cd kanban-app/ui && npm run test -- filter-editor.test.tsx` runs the new placeholder and `$project` dispatch scenarios and they pass.
- [x] No regressions: full `cargo test` and full `cd kanban-app/ui && npm run test` suites remain green.

## Tests

- [x] **Rust integration** — `swissarmyhammer-kanban/tests/filter_integration.rs` new scenarios:
  - `s20_filter_by_project` — setup, add project `auth`, add scoped + unscoped tasks, dispatch `list tasks filter "$auth"`, assert count = 1 and title matches.
  - `s21_project_combined_with_tag` — as above plus a second task with `#bug`, dispatch `"$auth && #bug"`, assert count = 1.
  - `s22_perspective_roundtrip_with_project_filter` — add perspective with `filter: "$auth"`, get → update `filter: "$auth || #urgent"` → get, assert persisted at each step.
  - Run: `cargo test -p swissarmyhammer-kanban --test filter_integration` — expect all 22 scenarios green.
- [x] **Frontend scenario** — `kanban-app/ui/src/components/filter-editor.test.tsx` new tests:
  - `"placeholder advertises $project sigil"` — render empty editor, find `.cm-placeholder`, assert its text contains `$`.
  - `"dispatches perspective.filter for $project expression"` — use the existing `getEditorView` helper, insert `$spatial-nav`, wait 400 ms, assert `mockInvoke` was called with `cmd: "perspective.filter"`, `args: { filter: "$spatial-nav", perspective_id: "p1" }`.
  - Run: `cd kanban-app/ui && npm run test -- filter-editor.test.tsx` — expect green.
- [x] **Frontend regression** — `cd kanban-app/ui && npm run test` — full suite green; the grammar tests (`lang-filter/__tests__/parser.test.ts`) remain untouched and still pass.
- [x] **Rust regression** — `cargo test -p swissarmyhammer-filter-expr -p swissarmyhammer-kanban` — no regressions in parser, evaluator, or task list unit tests.

## Workflow

- Use `/tdd` — write the failing `s20_filter_by_project` scenario and the failing `$spatial-nav dispatches` frontend test first. Watch them fail. Then add the placeholder change, the `$` hardcoded sigil completion, and the remaining integration scenarios until all tests pass.

## Notes / related

- The prior card 01KNWQ34B7864MV7MEX0YE4KGJ noted `$project` as "out of scope" because it assumed the DSL parser did not support `$`. Research for this card shows the parser already supports it (see `swissarmyhammer-filter-expr/src/parser.rs::atom_and_not` and its `project_*` tests) — the gap is purely UX wiring and missing integration/scenario coverage, exactly what this card fixes.
- This card is independent of 01KNWQ34B7864MV7MEX0YE4KGJ and can ship in either order. The frontend scenario tests mock `invoke`, so they do not require real project entities to be loaded.
