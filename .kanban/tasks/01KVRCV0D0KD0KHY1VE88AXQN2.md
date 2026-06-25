---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvzgxf1abt101sy7f03jfqa6
  text: 'Picked up. Research done: confirmed exact symbols in cm-mention-tooltip.ts (mentionAtPos, buildTooltipDom, MentionMeta, createMentionTooltips), and that buildScopedExtensions in mention-view.tsx wires only createMentionDecorations (no tooltip). use-mention-extensions.ts wires getTooltipInfra (cm-<type>-tooltip) for editable fields. Proceeding TDD: writing both failing tests first.'
  timestamp: 2026-06-25T13:53:36.298495+00:00
- actor: claude-code
  id: 01kvzh7dqw8w9qa5mn1barqtk3
  text: |-
    DONE (TDD RED->GREEN).

    Files changed:
    - apps/kanban-app/ui/src/lib/cm-mention-tooltip.ts — extracted exported `mentionTooltipAt(text,pos,lineFrom,prefix,cssClass,meta): {pos;end;dom}|null` running mentionAtPos + meta.get + buildTooltipDom; hoverTooltip source is now a thin wrapper delegating to it (byte-identical output for the editable path: same pos/end, above:true, same dom, null-on-miss).
    - apps/kanban-app/ui/src/components/mention-view.tsx — buildScopedExtensions now builds createMentionTooltips(r.prefix, `cm-${entityType}-tooltip`) and pushes tooltip.extension(metaMap) alongside the decoration extension, sharing the SAME metaMap (so unresolved placeholders apply to both). Import added.
    - apps/kanban-app/ui/src/lib/cm-mention-tooltip.test.ts (NEW) — 3 unit tests: header+title present; wrong-prefix => null; off-mention pos => null.
    - apps/kanban-app/ui/src/components/mention-view.test.tsx — vi.mock createMentionTooltips spy + test asserting it's called with ("^","cm-task-tooltip").

    TDD evidence:
    - RED unit: tsc reported `no exported member 'mentionTooltipAt'`. GREEN: 3/3 pass.
    - RED wiring: `wires createMentionTooltips ... × ... toHaveBeenCalledWith` (spy never called). GREEN after wiring: passes.

    Verification (apps/kanban-app/ui):
    - `npx tsc --noEmit` => clean (rc=0).
    - `npx vitest run cm-mention-tooltip mention-view use-mention-extensions` => 34 passed, 1 failed. The sole failure `extraCommands single mode right-click` is PRE-EXISTING: proved it fails on clean HEAD (git checkout HEAD of the 3 prod/test files, ran -t extraCommands => still fails), then restored. Out of scope per card.

    Invariants confirmed: cm-mention-decorations.ts has ZERO diff (MentionWidget.toDOM unchanged); no second tooltip mechanism; decoration pill text/color unchanged (existing MentionView + use-mention-extensions tests green); editable-field tooltip path unaffected. double-check agent verdict: PASS.
  timestamp: 2026-06-25T13:59:02.652348+00:00
- actor: wballard
  id: 01kvzhmrvnszqve1p6915q4aap
  text: |-
    Review resolution — FIX CORRECT, 0 blockers. Reviewer independently verified all 5 focus areas + all acceptance criteria: (1) `mentionTooltipAt` extraction behavior-preserving (3/3 unit tests: header+title present, wrong-prefix→null, off-mention→null; editable-field path unaffected); (2) wiring pushes BOTH deco + tooltip extensions in buildScopedExtensions reusing the same per-type metaMap, class `cm-${entityType}-tooltip`, for every pill type (new mention-view spy test asserts call `("^","cm-task-tooltip")`, RED→GREEN); (3) NO second tooltip mechanism — cm-mention-decorations.ts ZERO diff, MentionWidget.toDOM unchanged, no native title attr; (4) TDD genuine (new test file untracked, export didn't exist before / spy never called); (5) decoration pill text/color unchanged.

    1 warning + 3 nits WAIVED as pre-existing whole-file clarity noise (none defects in this diff): a sibling test duplicating the <Providers> hierarchy; two anonymous inline props types on MentionViewSingle/List; missing @returns JSDoc on createMentionTooltips.

    Pre-existing unrelated chromium failure `extraCommands single mode right-click` reproduced on clean HEAD by the reviewer (stash+restore) — NOT this task's regression. Verified: tsc clean; cm-mention-tooltip + new mention-view tooltip test + use-mention-extensions green. Moving to done.
  timestamp: 2026-06-25T14:06:20.021076+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffec80
project: pill-via-cm6
title: Mention pills have no hover tooltip — wire createMentionTooltips into MentionView
---
## What

**Bug**: Task dependency pills (the `depends_on` field) — and in fact *every* pill rendered on a card or in the inspector — show no hover tooltip. A dependency pill displays only `^<short>` (a 7-char id), so without a tooltip there is no way to see which task it points at. Hovering shows nothing.

**Root cause** — pills outside editable text are rendered by `MentionView` (`apps/kanban-app/ui/src/components/mention-view.tsx`). Its private `buildScopedExtensions` (mention-view.tsx:236) builds the CM6 extension set for each entity type but only wires the **decoration** extension (`createMentionDecorations`) — it never wires the **tooltip** extension (`createMentionTooltips`). The tooltip implementation already exists in `apps/kanban-app/ui/src/lib/cm-mention-tooltip.ts` and IS wired for *editable* text fields via `buildMentionExtensions` in `apps/kanban-app/ui/src/hooks/use-mention-extensions.ts:240` (`getTooltipInfra(...).extension(metaMap)`). So a `^ref` in a markdown body shows a rich hover tooltip, but the same reference rendered as a `depends_on` pill on the card does not.

The metadata needed is already present: `buildMentionMetaMap` (use-mention-extensions.ts:96) sets `description = title` for tasks (slug-field types) and `description = entity.description` for tags/actors. `buildTooltipDom` (cm-mention-tooltip.ts:32) renders a colored dot + `${prefix}${slug}` header + the `description` line. The existing test at `mention-view.test.tsx:239` even comments that the task title is "kept for the tooltip" — the intent was always there; only the wiring is missing.

### Fix approach

1. **Wire the tooltip into `MentionView`.** In `buildScopedExtensions` (mention-view.tsx), mirror the decoration block: build `createMentionTooltips(r.prefix, \`cm-${r.entityType}-tooltip\`)` and push `tooltip.extension(metaMap)` alongside the decoration extension, reusing the SAME `metaMap`. Reuse the existing `createMentionTooltips` — do NOT add a second tooltip mechanism (e.g. a native `title` attribute on `MentionWidget`); the body-text pills and field pills must share one tooltip implementation.

2. **Make the tooltip content testable.** CM6 `hoverTooltip` cannot be triggered in jsdom (no layout → no `posAtCoords`), so extract the pure resolution logic from the hover source in `cm-mention-tooltip.ts` into an exported function:

   ```ts
   export function mentionTooltipAt(
     text: string, pos: number, lineFrom: number,
     prefix: string, cssClass: string, meta: Map<string, MentionMeta>,
   ): { pos: number; end: number; dom: HTMLElement } | null
   ```

   It runs `mentionAtPos` + `meta.get` + `buildTooltipDom` and returns the tooltip payload (or null). The `hoverTooltip` source inside `createMentionTooltips` then becomes a thin wrapper that calls it. No behavior change for the existing editable-field path.

### Out of scope
- The project-pill empty-slug bug (tracked separately as `^pfpk1gg`). This task assumes a pill resolves; it adds the tooltip to whatever pills `MentionView` already renders.
- Restyling the tooltip DOM. Keep `buildTooltipDom`'s current layout.

## Acceptance Criteria
- [ ] `mentionTooltipAt` returns a payload whose `dom.textContent` contains the task title (`description`) and the `^<short>` header when the position is inside a task mention; returns `null` when the position is off any known mention.
- [ ] `buildScopedExtensions` wires a tooltip extension for every rendered pill type (the same `createMentionTooltips(prefix, "cm-<type>-tooltip")` used by editable fields), reusing the per-type `metaMap`.
- [ ] Decoration rendering (pill text/color) is unchanged — existing `MentionView` and `use-mention-extensions` tests still pass.
- [ ] No second tooltip mechanism is introduced (`MentionWidget.toDOM` is unchanged).

## Tests
- [ ] New unit test `apps/kanban-app/ui/src/lib/cm-mention-tooltip.test.ts`: build a meta map `{ "28rfp1r": { color: "00ff00", displayName: "28rfp1r", description: "Long Sentence-Like Task Title" } }`; call `mentionTooltipAt("^28rfp1r", 3, 0, "^", "cm-task-tooltip", meta)` and assert the returned `dom.textContent` includes both `^28rfp1r` and `Long Sentence-Like Task Title`. Assert `mentionTooltipAt("^28rfp1r", 3, 0, "#", "cm-tag-tooltip", meta)` (wrong prefix) and a position off the mention both return `null`. This fails before the export exists / passes after.
- [ ] In `apps/kanban-app/ui/src/components/mention-view.test.tsx`, add a test that `vi.mock`s `@/lib/cm-mention-tooltip` with a spied `createMentionTooltips` (returning `{ extension: () => [] }`), renders `<MentionView entityType="task" id="01KT4CNAYW7JG0X8F8W28RFP1R" />` against the existing task fixture (slugField `short_id`), and asserts `createMentionTooltips` was called with `("^", "cm-task-tooltip")`. This fails before the wiring is added (spy never called) and passes after.
- [ ] Run: `cd apps/kanban-app/ui && npm test -- cm-mention-tooltip mention-view use-mention-extensions` — all pass.
- [ ] Run the full UI suite: `cd apps/kanban-app/ui && npm test` — no regressions.

## Workflow
- Use `/tdd` — write the `cm-mention-tooltip.test.ts` unit test and the `mention-view.test.tsx` wiring test first (both fail: the export doesn't exist and the spy is never called), then extract `mentionTooltipAt` and wire `createMentionTooltips` into `buildScopedExtensions` so they pass. #ui

## Review Findings (2026-06-25 08:00)

Scope: working tree vs HEAD (`090d27abf`). Touched: `cm-mention-tooltip.ts`, `mention-view.tsx`, new `cm-mention-tooltip.test.ts`, `mention-view.test.tsx`. Engine counts: 0 blockers, 1 warning, 3 nits (4 confirmed, 1 refuted).

### In-scope verification (all PASS — the fix is correct)
- [x] Invariant: `cm-mention-decorations.ts` has ZERO diff — `MentionWidget.toDOM` unchanged, no native `title`, no second tooltip mechanism (git confirmed, not in diff-stat).
- [x] `mentionTooltipAt` extraction is behavior-preserving: 3/3 unit tests green — header `^28rfp1r` + title both present; wrong-prefix → null; off-mention → null.
- [x] Wiring test "wires createMentionTooltips for the rendered task pill type" green — `createMentionTooltips` called with `("^", "cm-task-tooltip")`; previously the spy was never called (RED→GREEN confirmed by implementer).
- [x] Decoration rendering (pill text/color) unchanged — existing MentionView pill tests green.
- [x] `npx tsc --noEmit` clean (exit 0).
- [x] Pre-existing failure `extraCommands single mode right-click` (mention-view.test.tsx:567, show-context-menu) reproduced on clean HEAD with task changes stashed — NOT this task's regression, DISREGARDED.

### Warnings (clarity, whole-file — not a fix defect)
- [ ] `apps/kanban-app/ui/src/components/mention-view.test.tsx:168` — test 'renders one pill scope per item, nested inside the parent field row' duplicates the `<Providers>` hierarchy (SpatialFocusProvider → FocusLayer → EntityFocusProvider → TooltipProvider) verbatim instead of using the extracted `Providers` helper; will drift when the hierarchy changes.

### Nits (clarity, whole-file — not a fix defect)
- [ ] `apps/kanban-app/ui/src/components/mention-view.tsx:277` — `MentionViewSingle` uses an anonymous inline props type instead of a named `MentionViewSingleProps` interface.
- [ ] `apps/kanban-app/ui/src/components/mention-view.tsx:297` — `MentionViewList` uses an anonymous inline props type instead of a named `MentionViewListProps` interface.
- [ ] `apps/kanban-app/ui/src/lib/cm-mention-tooltip.ts:98` — public `createMentionTooltips` JSDoc lacks a `@returns` clause documenting the `{ metaFacet, extension(meta) }` return shape.