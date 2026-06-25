---
assignees:
- claude-code
position_column: todo
position_ordinal: fc80
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